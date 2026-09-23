//! Faces bundled into the binary: default player skins and mobs.

use crate::image::Image;

#[rustfmt::skip]
mod builtin_data;
pub use builtin_data::BUILTINS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Mob,
}

pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub w: u32,
    pub h: u32,
    pub rgba: &'static [u8],
}

impl Builtin {
    pub fn face(&self) -> Image {
        Image::from_rgba(self.w, self.h, self.rgba)
    }
}

/// Looks up a builtin by id, ignoring case and treating `-` like `_`.
pub fn find(id: &str) -> Option<&'static Builtin> {
    let id = id.to_ascii_lowercase().replace('-', "_");
    BUILTINS.iter().find(|b| b.id == id)
}

pub fn random() -> &'static Builtin {
    &BUILTINS[fastrand::usize(..BUILTINS.len())]
}

/// Default skins in the order Minecraft's `DefaultPlayerSkin` lists them.
const DEFAULT_SKIN_ORDER: [&str; 9] = [
    "alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri",
];

/// The default skin Minecraft gives a player who has no custom skin:
/// `floorMod(uuid.hashCode(), 18)` over 9 slim skins followed by the same 9 wide ones.
/// Returns the skin and whether it is the slim model.
pub fn default_skin(uuid: u128) -> (&'static Builtin, bool) {
    let hilo = ((uuid >> 64) as u64) ^ (uuid as u64);
    let hash = ((hilo >> 32) as i32) ^ (hilo as i32);
    let idx = hash.rem_euclid(18) as usize;
    (
        find(DEFAULT_SKIN_ORDER[idx % 9]).expect("default skin is bundled"),
        idx < 9,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtins_are_well_formed() {
        for b in BUILTINS {
            assert_eq!(b.rgba.len(), (b.w * b.h * 4) as usize, "{}", b.id);
            assert!(b.rgba.chunks(4).any(|p| p[3] > 0), "{} is empty", b.id);
        }
    }

    #[test]
    fn find_is_lenient() {
        assert_eq!(find("Steve").unwrap().id, "steve");
        assert_eq!(find("wither-skeleton").unwrap().id, "wither_skeleton");
        assert!(find("nobody").is_none());
    }

    #[test]
    fn default_skin_matches_java_hash() {
        // Java: new UUID(0, 0).hashCode() == 0 -> index 0 (slim alex).
        assert_eq!(default_skin(0).0.id, "alex");
        assert!(default_skin(0).1);
        // hashCode == 15 -> index 15 (wide steve).
        let (skin, slim) = default_skin(15);
        assert_eq!((skin.id, slim), ("steve", false));
        // Negative hash -1 -> floorMod(-1, 18) == 17 (wide zuri).
        let (skin, slim) = default_skin(0xffff_ffff);
        assert_eq!((skin.id, slim), ("zuri", false));
    }
}
