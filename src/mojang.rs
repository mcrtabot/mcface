//! Mojang API access (name -> UUID -> profile -> skin) with a small file cache.

use base64::Engine as _;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const NAME_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const PROFILE_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug)]
pub enum Error {
    NotFound(String),
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotFound(m) | Error::Other(m) => f.write_str(m),
        }
    }
}

pub struct Profile {
    pub uuid: u128,
    pub name: String,
    /// None when the player has no custom skin.
    pub skin_url: Option<String>,
    pub slim: bool,
}

pub struct Client {
    agent: ureq::Agent,
    cache: Option<PathBuf>,
}

impl Client {
    pub fn new(use_cache: bool) -> Client {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .user_agent(concat!("mcface/", env!("CARGO_PKG_VERSION")))
            .build();
        Client {
            agent: config.into(),
            cache: use_cache.then(cache_dir),
        }
    }

    /// Resolves a player name to (uuid, name with correct capitalisation).
    pub fn lookup_name(&self, name: &str) -> Result<(u128, String), Error> {
        let key = format!("name/{}.json", name.to_ascii_lowercase());
        let primary = format!("https://api.mojang.com/users/profiles/minecraft/{name}");
        let fallback =
            format!("https://api.minecraftservices.com/minecraft/profile/lookup/name/{name}");
        let fetch = || self.get(&primary).or_else(|_| self.get(&fallback));
        let body = self
            .cached(&key, Some(NAME_TTL), fetch)?
            .ok_or_else(|| Error::NotFound(format!("player '{name}' not found")))?;
        let v: Value = parse_json(&body)?;
        let uuid = v["id"]
            .as_str()
            .and_then(parse_uuid)
            .ok_or_else(|| bad_response("id"))?;
        let name = v["name"]
            .as_str()
            .ok_or_else(|| bad_response("name"))?
            .to_string();
        Ok((uuid, name))
    }

    pub fn profile(&self, uuid: u128) -> Result<Profile, Error> {
        let key = format!("profile/{uuid:032x}.json");
        let url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid:032x}");
        let body = self
            .cached(&key, Some(PROFILE_TTL), || self.get(&url))?
            .ok_or_else(|| Error::NotFound(format!("no player with UUID {}", format_uuid(uuid))))?;
        parse_profile(&body)
    }

    pub fn skin(&self, url: &str) -> Result<Vec<u8>, Error> {
        // Texture URLs are content-addressed, so a cached copy never goes stale.
        let hash = url.rsplit('/').next().unwrap_or_default();
        let key = if !hash.is_empty() && hash.chars().all(|c| c.is_ascii_alphanumeric()) {
            format!("skin/{hash}.png")
        } else {
            format!("skin/{:016x}.png", fnv1a(url.as_bytes()))
        };
        let url = url.replacen("http://", "https://", 1);
        self.cached(&key, None, || self.get(&url))?
            .ok_or_else(|| Error::Other(format!("skin texture not found: {url}")))
    }

    /// GET returning Ok(None) for "no such resource" (204/404).
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>, Error> {
        let mut resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| Error::Other(format!("request failed: {e}")))?;
        match resp.status().as_u16() {
            200 => resp
                .body_mut()
                .read_to_vec()
                .map(Some)
                .map_err(|e| Error::Other(format!("reading response failed: {e}"))),
            204 | 404 => Ok(None),
            429 => Err(Error::Other(
                "rate limited by the Mojang API (HTTP 429); try again in a minute".into(),
            )),
            s => Err(Error::Other(format!("unexpected HTTP {s} from {url}"))),
        }
    }

    /// Serves `key` from the cache when fresh, otherwise fetches and stores it.
    /// If the fetch fails for a network reason, a stale cached copy is used instead.
    fn cached(
        &self,
        key: &str,
        ttl: Option<Duration>,
        fetch: impl FnOnce() -> Result<Option<Vec<u8>>, Error>,
    ) -> Result<Option<Vec<u8>>, Error> {
        let Some(dir) = &self.cache else {
            return fetch();
        };
        let path = dir.join(key);
        let stale = match (
            std::fs::read(&path),
            std::fs::metadata(&path).and_then(|m| m.modified()),
        ) {
            (Ok(bytes), Ok(mtime)) => {
                let age = SystemTime::now().duration_since(mtime).unwrap_or_default();
                if ttl.is_none_or(|ttl| age < ttl) {
                    return Ok(Some(bytes));
                }
                Some(bytes)
            }
            _ => None,
        };
        match fetch() {
            Ok(Some(bytes)) => {
                store(&path, &bytes);
                Ok(Some(bytes))
            }
            Ok(None) => {
                let _ = std::fs::remove_file(&path);
                Ok(None)
            }
            Err(e) => match stale {
                Some(bytes) => {
                    eprintln!("mcface: {e}; using cached data");
                    Ok(Some(bytes))
                }
                None => Err(e),
            },
        }
    }
}

pub fn cache_dir() -> PathBuf {
    match std::env::var_os("MCFACE_CACHE_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => std::env::temp_dir().join("mcface"),
    }
}

/// Best-effort atomic write; caching failures never fail the command.
fn store(path: &std::path::Path, bytes: &[u8]) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn parse_json(body: &[u8]) -> Result<Value, Error> {
    serde_json::from_slice(body)
        .map_err(|e| Error::Other(format!("invalid JSON from Mojang API: {e}")))
}

fn bad_response(field: &str) -> Error {
    Error::Other(format!(
        "unexpected Mojang API response (missing '{field}')"
    ))
}

fn parse_profile(body: &[u8]) -> Result<Profile, Error> {
    let v = parse_json(body)?;
    let uuid = v["id"]
        .as_str()
        .and_then(parse_uuid)
        .ok_or_else(|| bad_response("id"))?;
    let name = v["name"]
        .as_str()
        .ok_or_else(|| bad_response("name"))?
        .to_string();
    let textures = v["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["name"] == "textures")
        .and_then(|p| p["value"].as_str());
    let (mut skin_url, mut slim) = (None, false);
    if let Some(encoded) = textures {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| bad_response("textures"))?;
        let t = parse_json(&decoded)?;
        let skin = &t["textures"]["SKIN"];
        skin_url = skin["url"].as_str().map(str::to_string);
        slim = skin["metadata"]["model"] == "slim";
    }
    Ok(Profile {
        uuid,
        name,
        skin_url,
        slim,
    })
}

/// Accepts 32 hex digits, with or without the usual 8-4-4-4-12 hyphens.
pub fn parse_uuid(s: &str) -> Option<u128> {
    let hex: String = if s.len() == 36 {
        let groups: Vec<&str> = s.split('-').collect();
        if groups.iter().map(|g| g.len()).collect::<Vec<_>>() != [8, 4, 4, 4, 12] {
            return None;
        }
        groups.concat()
    } else {
        s.to_string()
    };
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

pub fn format_uuid(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

pub fn is_valid_name(s: &str) -> bool {
    (1..=16).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_forms() {
        let plain = "069a79f444e94726a5befca90e38aaf5";
        let dashed = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
        assert_eq!(parse_uuid(plain), parse_uuid(dashed));
        assert_eq!(format_uuid(parse_uuid(plain).unwrap()), dashed);
        assert_eq!(parse_uuid("069a79f444e94726a5befca90e38aaf"), None);
        assert_eq!(parse_uuid("069a79f4-44e94726-a5be-fca90e38aaf5"), None);
        assert_eq!(parse_uuid("zz9a79f444e94726a5befca90e38aaf5"), None);
    }

    #[test]
    fn names() {
        assert!(is_valid_name("Notch"));
        assert!(is_valid_name("jeb_"));
        assert!(!is_valid_name("../etc"));
        assert!(!is_valid_name("a_name_that_is_too_long"));
    }

    #[test]
    fn profile_textures() {
        let textures = r#"{"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#;
        let body = format!(
            r#"{{"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch","properties":[{{"name":"textures","value":"{}"}}]}}"#,
            base64::engine::general_purpose::STANDARD.encode(textures)
        );
        let p = parse_profile(body.as_bytes()).unwrap();
        assert_eq!(p.name, "Notch");
        assert_eq!(
            p.skin_url.as_deref(),
            Some("http://textures.minecraft.net/texture/abc")
        );
        assert!(p.slim);
    }
}
