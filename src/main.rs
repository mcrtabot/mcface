use clap::Parser;
use mcface::builtin::{self, BUILTINS, Kind};
use mcface::image::Image;
use mcface::mojang::{self, Client};
use mcface::render::{self, ColorLevel, Mode};
use mcface::skin;
use std::io::Write as _;
use std::process::ExitCode;

/// Show Minecraft skin faces in the terminal.
#[derive(Parser)]
#[command(version, after_help = AFTER_HELP)]
struct Cli {
    /// Player names, UUIDs (with or without hyphens), or built-in characters as @name.
    targets: Vec<String>,

    /// How pixels are drawn.
    #[arg(short, long, value_enum, default_value_t = Mode::Block)]
    mode: Mode,

    /// Integer scale factor.
    #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=32))]
    scale: u32,

    /// Print the name under each face.
    #[arg(short, long)]
    name: bool,

    /// Leave out the hat/overlay layer.
    #[arg(long)]
    no_overlay: bool,

    /// Colour support. `auto` checks NO_COLOR, COLORTERM and TERM.
    #[arg(long, value_name = "LEVEL", default_value = "auto",
          value_parser = ["auto", "truecolor", "256", "16", "none"])]
    color: String,

    /// Print profile info and face pixels as JSON (one object per line) instead of drawing.
    #[arg(long, conflicts_with = "png")]
    json: bool,

    /// Write the face as a PNG (scaled by --scale) instead of drawing.
    /// With several targets, `{}` in the path is replaced by each name.
    #[arg(long, value_name = "PATH")]
    png: Option<String>,

    /// Add a random built-in character.
    #[arg(short, long)]
    random: bool,

    /// List built-in characters and exit.
    #[arg(short, long)]
    list: bool,

    /// Show every built-in character with its name (implies --name).
    #[arg(short, long)]
    all: bool,

    /// Neither read nor write the local cache.
    #[arg(long)]
    no_cache: bool,
}

const AFTER_HELP: &str = "\
Examples:
  mcface Notch                   player by name
  mcface 069a79f4-44e9-4726-a5be-fca90e38aaf5
  mcface @steve @creeper -m half -s 2 --name
  mcface --random
  mcface --all -m half           every built-in face with its name
  mcface jeb_ --png face.png -s 16

Cache: $MCFACE_CACHE_DIR, default <tmp>/mcface (names 24h, profiles 1h, skins forever).";

fn color_level(s: &str) -> ColorLevel {
    match s {
        "truecolor" => ColorLevel::True,
        "256" => ColorLevel::Ansi256,
        "16" => ColorLevel::Ansi16,
        "none" => ColorLevel::None,
        _ => render::detect_color(),
    }
}

struct Resolved {
    input: String,
    name: String,
    face: Image,
    builtin: Option<&'static str>,
    player: Option<PlayerInfo>,
}

struct PlayerInfo {
    uuid: u128,
    skin_url: Option<String>,
    slim: bool,
    /// Set when the player has no custom skin and Minecraft's default applies.
    default_skin: Option<&'static str>,
}

fn resolve(client: &Client, target: &str, overlay: bool) -> Result<Resolved, String> {
    if let Some(id) = target.strip_prefix('@') {
        let b =
            builtin::find(id).ok_or_else(|| format!("unknown character '@{id}' (see --list)"))?;
        return Ok(from_builtin(target, b));
    }
    let uuid = match mojang::parse_uuid(target) {
        Some(uuid) => uuid,
        None if mojang::is_valid_name(target) => {
            client.lookup_name(target).map_err(|e| e.to_string())?.0
        }
        None => {
            return Err(
                "not a valid player name or UUID (use @name for built-in characters)".into(),
            );
        }
    };
    let profile = client.profile(uuid).map_err(|e| e.to_string())?;
    let (face, slim, default_skin) = match &profile.skin_url {
        Some(url) => {
            let png = client.skin(url).map_err(|e| e.to_string())?;
            let skin = Image::decode_png(&png)?;
            (skin::face(&skin, overlay)?, profile.slim, None)
        }
        None => {
            let (b, slim) = builtin::default_skin(uuid);
            (b.face(), slim, Some(b.id))
        }
    };
    Ok(Resolved {
        input: target.to_string(),
        name: profile.name,
        face,
        builtin: None,
        player: Some(PlayerInfo {
            uuid: profile.uuid,
            skin_url: profile.skin_url,
            slim,
            default_skin,
        }),
    })
}

fn from_builtin(input: &str, b: &'static builtin::Builtin) -> Resolved {
    Resolved {
        input: input.to_string(),
        name: b.name.to_string(),
        face: b.face(),
        builtin: Some(b.id),
        player: None,
    }
}

fn main() -> ExitCode {
    let mut cli = Cli::parse();
    cli.name |= cli.all;

    if cli.list {
        print_list();
        return ExitCode::SUCCESS;
    }
    let mut targets = cli.targets.clone();
    if cli.all {
        targets.extend(BUILTINS.iter().map(|b| format!("@{}", b.id)));
    }
    if cli.random {
        targets.push(format!("@{}", builtin::random().id));
    }
    if targets.is_empty() {
        eprintln!("mcface: give a player name, UUID or @character (try --help or --list)");
        return ExitCode::from(2);
    }
    if let Some(path) = &cli.png
        && targets.len() > 1
        && !path.contains("{}")
    {
        eprintln!(
            "mcface: --png with several targets needs `{{}}` in the path, e.g. faces/{{}}.png"
        );
        return ExitCode::from(2);
    }

    let client = Client::new(!cli.no_cache);
    let overlay = !cli.no_overlay;
    // Every target needs up to three sequential requests, so run targets in parallel.
    let results: Vec<Result<Resolved, String>> = std::thread::scope(|s| {
        let handles: Vec<_> = targets
            .iter()
            .map(|t| s.spawn(|| resolve(&client, t, overlay)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("internal error".into())))
            .collect()
    });

    let mut ok = Vec::new();
    let mut failed = false;
    for (target, result) in targets.iter().zip(results) {
        match result {
            Ok(r) => ok.push(r),
            Err(e) => {
                failed = true;
                if cli.json {
                    println!("{}", serde_json::json!({ "input": target, "error": e }));
                } else {
                    eprintln!("mcface: {target}: {e}");
                }
            }
        }
    }

    if cli.json {
        for r in &ok {
            println!("{}", to_json(r));
        }
    } else if let Some(path) = &cli.png {
        for r in &ok {
            let path = path.replace("{}", &r.name.replace(' ', "_"));
            if let Err(e) = std::fs::write(&path, r.face.scale(cli.scale).encode_png()) {
                eprintln!("mcface: {path}: {e}");
                failed = true;
            }
        }
    } else if !ok.is_empty() {
        let level = color_level(&cli.color);
        print_faces(&ok, &cli, level);
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_faces(faces: &[Resolved], cli: &Cli, level: ColorLevel) {
    const GAP: usize = 2;
    let blocks: Vec<(render::Rendered, &str)> = faces
        .iter()
        .map(|r| {
            (
                render::render(&r.face.scale(cli.scale), cli.mode, level),
                r.name.as_str(),
            )
        })
        .collect();
    let col_width = |(face, name): &(render::Rendered, &str)| {
        if cli.name {
            face.width.max(name.len())
        } else {
            face.width
        }
    };

    // Wrap into rows that fit the terminal.
    let max = term_width().unwrap_or(usize::MAX);
    let mut rows: Vec<Vec<&(render::Rendered, &str)>> = vec![Vec::new()];
    let mut used = 0;
    for b in &blocks {
        let w = col_width(b);
        let row = rows.last_mut().unwrap();
        if !row.is_empty() && used + GAP + w > max {
            rows.push(Vec::new());
            used = 0;
        }
        let row = rows.last_mut().unwrap();
        used += if row.is_empty() { w } else { GAP + w };
        row.push(b);
    }

    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let height = row.iter().map(|(f, _)| f.lines.len()).max().unwrap_or(0);
        for y in 0..height {
            let mut line = String::new();
            for (j, b) in row.iter().enumerate() {
                let (face, _) = b;
                let w = col_width(b);
                let left = (w - face.width) / 2;
                if j > 0 {
                    line.push_str(&" ".repeat(GAP));
                }
                line.push_str(&" ".repeat(left));
                match face.lines.get(y) {
                    Some(l) => line.push_str(l),
                    None => line.push_str(&" ".repeat(face.width)),
                }
                line.push_str(&" ".repeat(w - face.width - left));
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        if cli.name {
            let mut line = String::new();
            for (j, b) in row.iter().enumerate() {
                let w = col_width(b);
                if j > 0 {
                    line.push_str(&" ".repeat(GAP));
                }
                line.push_str(&format!("{:^w$}", b.1));
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
}

fn to_json(r: &Resolved) -> serde_json::Value {
    let pixels: Vec<String> = r
        .face
        .px
        .chunks(r.face.w as usize)
        .map(|row| {
            row.iter()
                .map(|p| format!("{:02x}{:02x}{:02x}{:02x}", p[0], p[1], p[2], p[3]))
                .collect()
        })
        .collect();
    let mut v = serde_json::json!({ "input": r.input, "name": r.name });
    if let Some(id) = r.builtin {
        v["kind"] = "builtin".into();
        v["id"] = id.into();
    }
    if let Some(p) = &r.player {
        v["kind"] = "player".into();
        v["uuid"] = mojang::format_uuid(p.uuid).into();
        v["skin_url"] = p.skin_url.clone().into();
        v["model"] = (if p.slim { "slim" } else { "classic" }).into();
        v["default_skin"] = p.default_skin.into();
    }
    v["face"] = serde_json::json!({ "width": r.face.w, "height": r.face.h, "pixels": pixels });
    v
}

fn print_list() {
    for (kind, title) in [(Kind::Player, "Default skins"), (Kind::Mob, "Mobs")] {
        println!("{title}:");
        for b in BUILTINS.iter().filter(|b| b.kind == kind) {
            println!("  @{:<16} {}", b.id, b.name);
        }
    }
}

/// Terminal width for wrapping: $COLUMNS if set, else the size of the stdout terminal.
fn term_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .filter(|&c| c > 0)
        .or_else(tty_width)
}

#[cfg(unix)]
fn tty_width() -> Option<usize> {
    use std::io::IsTerminal as _;
    if !std::io::stdout().is_terminal() {
        return None;
    }
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: TIOCGWINSZ fills a winsize struct we own.
    let rc = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) };
    (rc == 0 && ws.ws_col > 0).then_some(ws.ws_col as usize)
}

#[cfg(not(unix))]
fn tty_width() -> Option<usize> {
    None
}
