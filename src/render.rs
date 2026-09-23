//! Rendering images as terminal text.

use crate::image::{Image, Rgba};
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Mode {
    /// Two full-block characters `██` per pixel, coloured with the foreground.
    Block,
    /// Two spaces per pixel, coloured with the background.
    Bg,
    /// Half blocks `▀`: two pixels stacked in one cell, so pixels come out square.
    Half,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorLevel {
    True,
    Ansi256,
    Ansi16,
    /// No colour: shades pixels with ASCII characters by brightness.
    None,
}

/// Picks the richest colour support the terminal advertises.
pub fn detect_color() -> ColorLevel {
    let var = |k: &str| std::env::var(k).unwrap_or_default();
    if !var("NO_COLOR").is_empty() {
        return ColorLevel::None;
    }
    let colorterm = var("COLORTERM").to_ascii_lowercase();
    let term = var("TERM").to_ascii_lowercase();
    let program = var("TERM_PROGRAM");
    if colorterm == "truecolor"
        || colorterm == "24bit"
        || term.contains("truecolor")
        || term.contains("direct")
    {
        return ColorLevel::True;
    }
    if ["iTerm.app", "WezTerm", "ghostty", "vscode"].contains(&program.as_str()) {
        return ColorLevel::True;
    }
    if term.contains("256") {
        return ColorLevel::Ansi256;
    }
    if term == "dumb" {
        return ColorLevel::None;
    }
    ColorLevel::Ansi16
}

pub struct Rendered {
    pub lines: Vec<String>,
    /// Width of every line in terminal cells.
    pub width: usize,
}

pub fn render(img: &Image, mode: Mode, level: ColorLevel) -> Rendered {
    let opaque = |x: u32, y: u32| -> Option<Rgba> {
        (y < img.h).then(|| img.get(x, y)).filter(|p| p[3] >= 128)
    };
    let mut lines = Vec::new();
    let (step, width) = match mode {
        Mode::Half => (2, img.w as usize),
        Mode::Block | Mode::Bg => (1, img.w as usize * 2),
    };
    for y in (0..img.h).step_by(step) {
        let mut line = Line::new(level);
        for x in 0..img.w {
            let px = opaque(x, y);
            match mode {
                Mode::Block => match px {
                    Some(c) => line.cell(Some(c), None, "██", shade(&[c]).repeat(2)),
                    None => line.cell(None, None, "  ", "  ".into()),
                },
                Mode::Bg => match px {
                    Some(c) => line.cell(None, Some(c), "  ", shade(&[c]).repeat(2)),
                    None => line.cell(None, None, "  ", "  ".into()),
                },
                Mode::Half => match (px, opaque(x, y + 1)) {
                    (Some(t), Some(b)) => line.cell(Some(t), Some(b), "▀", shade(&[t, b])),
                    (Some(t), None) => line.cell(Some(t), None, "▀", shade(&[t])),
                    (None, Some(b)) => line.cell(Some(b), None, "▄", shade(&[b])),
                    (None, None) => line.cell(None, None, " ", " ".into()),
                },
            }
        }
        lines.push(line.finish());
    }
    Rendered { lines, width }
}

/// Builds one line, emitting SGR codes only when the colours change.
struct Line {
    level: ColorLevel,
    out: String,
    fg: Option<String>,
    bg: Option<String>,
    styled: bool,
}

impl Line {
    fn new(level: ColorLevel) -> Line {
        Line {
            level,
            out: String::new(),
            fg: None,
            bg: None,
            styled: false,
        }
    }

    /// `fg`/`bg` of None mean the terminal default. `mono` is the text used without colour.
    fn cell(&mut self, fg: Option<Rgba>, bg: Option<Rgba>, text: &str, mono: String) {
        if self.level == ColorLevel::None {
            self.out.push_str(&mono);
            return;
        }
        let fg = fg.map(|c| sgr(c, self.level, false));
        let bg = bg.map(|c| sgr(c, self.level, true));
        let mut codes = Vec::new();
        // Only the background is visible for spaces, so the foreground can stay as is.
        if fg != self.fg && !text.trim().is_empty() {
            codes.push(fg.clone().unwrap_or_else(|| "39".into()));
            self.fg = fg;
        }
        if bg != self.bg {
            codes.push(bg.clone().unwrap_or_else(|| "49".into()));
            self.bg = bg;
        }
        if !codes.is_empty() {
            write!(self.out, "\x1b[{}m", codes.join(";")).unwrap();
            self.styled = true;
        }
        self.out.push_str(text);
    }

    fn finish(mut self) -> String {
        if self.styled {
            self.out.push_str("\x1b[0m");
        }
        self.out
    }
}

fn sgr(c: Rgba, level: ColorLevel, bg: bool) -> String {
    let [r, g, b, _] = c;
    match level {
        ColorLevel::True => format!("{};2;{r};{g};{b}", if bg { 48 } else { 38 }),
        ColorLevel::Ansi256 => format!("{};5;{}", if bg { 48 } else { 38 }, to_256(r, g, b)),
        ColorLevel::Ansi16 => {
            let i = to_16(r, g, b);
            let base = match (bg, i < 8) {
                (false, true) => 30,
                (false, false) => 90,
                (true, true) => 40,
                (true, false) => 100,
            };
            (base + i % 8).to_string()
        }
        ColorLevel::None => String::new(),
    }
}

fn dist(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    // Rough perceptual weighting.
    let (dr, dg, db) = (a.0 - b.0, a.1 - b.1, a.2 - b.2);
    2 * dr * dr + 4 * dg * dg + 3 * db * db
}

fn to_256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |v: u8| {
        (0..6)
            .min_by_key(|&i| (LEVELS[i] - v as i32).abs())
            .unwrap()
    };
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let cube_idx = 16 + 36 * ri + 6 * gi + bi;
    let avg = (r as i32 + g as i32 + b as i32) / 3;
    let gi2 = ((avg - 8).max(0) / 10).min(23);
    let gv = 8 + 10 * gi2;
    let target = (r as i32, g as i32, b as i32);
    if dist(target, (gv, gv, gv)) < dist(target, cube) {
        (232 + gi2) as u8
    } else {
        cube_idx as u8
    }
}

/// xterm's default 16-colour palette.
const PALETTE16: [(i32, i32, i32); 16] = [
    (0, 0, 0),
    (205, 0, 0),
    (0, 205, 0),
    (205, 205, 0),
    (0, 0, 238),
    (205, 0, 205),
    (0, 205, 205),
    (229, 229, 229),
    (127, 127, 127),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (92, 92, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

fn to_16(r: u8, g: u8, b: u8) -> u8 {
    let t = (r as i32, g as i32, b as i32);
    (0..16).min_by_key(|&i| dist(t, PALETTE16[i])).unwrap() as u8
}

/// ASCII shade for the average brightness of the given pixels (bright = dense).
fn shade(px: &[Rgba]) -> String {
    const RAMP: &[u8] = b".:-=+*#%@";
    let lum: u32 = px
        .iter()
        .map(|p| (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000)
        .sum();
    let lum = lum / px.len() as u32;
    let i = (lum as usize * RAMP.len() / 256).min(RAMP.len() - 1);
    (RAMP[i] as char).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker() -> Image {
        let mut img = Image::new(2, 2);
        img.set(0, 0, [255, 0, 0, 255]);
        img.set(1, 1, [0, 0, 255, 255]);
        img
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                chars.by_ref().find(|&c| c == 'm');
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn block_mode_layout() {
        let r = render(&checker(), Mode::Block, ColorLevel::True);
        assert_eq!(r.width, 4);
        assert_eq!(r.lines[0], "\x1b[38;2;255;0;0m██  \x1b[0m");
        assert_eq!(strip_ansi(&r.lines[1]), "  ██");
    }

    #[test]
    fn bg_mode_resets_background_for_transparent_pixels() {
        let r = render(&checker(), Mode::Bg, ColorLevel::True);
        assert_eq!(r.lines[0], "\x1b[48;2;255;0;0m  \x1b[49m  \x1b[0m");
    }

    #[test]
    fn half_mode_packs_two_rows() {
        let r = render(&checker(), Mode::Half, ColorLevel::True);
        assert_eq!((r.lines.len(), r.width), (1, 2));
        assert_eq!(strip_ansi(&r.lines[0]), "▀▄");
    }

    #[test]
    fn mono_uses_shades() {
        let r = render(&checker(), Mode::Block, ColorLevel::None);
        assert!(!r.lines[0].contains('\x1b'));
        assert_eq!(r.lines[0].chars().count(), 4);
    }

    #[test]
    fn palette_mapping() {
        assert_eq!(to_256(255, 0, 0), 196);
        assert_eq!(to_256(128, 128, 128), 244);
        assert_eq!(to_16(250, 10, 10), 9);
        assert_eq!(sgr([0, 0, 0, 255], ColorLevel::Ansi16, true), "40");
    }
}
