//! Extracting the face from a player skin texture.

use crate::image::Image;

/// Cuts the 8x8 face (plus the hat layer when `overlay` is set) out of a skin.
/// Handles 64x64 and legacy 64x32 skins, and HD skins that are integer multiples.
pub fn face(skin: &Image, overlay: bool) -> Result<Image, String> {
    let legacy = skin.w == skin.h * 2;
    if skin.w < 64 || !skin.w.is_multiple_of(64) || !(legacy || skin.w == skin.h) {
        return Err(format!("unsupported skin size {}x{}", skin.w, skin.h));
    }
    let k = skin.w / 64;
    let mut face = skin.crop(8 * k, 8 * k, 8 * k, 8 * k)?;
    // Minecraft always renders the base head layer opaque.
    face.px.iter_mut().for_each(|p| p[3] = 255);
    if overlay && !(legacy && hat_area_is_opaque(skin, k)) {
        face.over(&skin.crop(40 * k, 8 * k, 8 * k, 8 * k)?, 0, 0);
    }
    Ok(face)
}

/// Legacy skins often filled the hat area with a solid colour; Minecraft ignores
/// the hat layer entirely in that case (the "Notch transparency hack").
fn hat_area_is_opaque(skin: &Image, k: u32) -> bool {
    (0..16 * k).all(|y| (32 * k..64 * k).all(|x| skin.get(x, y)[3] == 255))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin(w: u32, h: u32, fill: [u8; 4]) -> Image {
        let mut img = Image::new(w, h);
        img.px.iter_mut().for_each(|p| *p = fill);
        img
    }

    #[test]
    fn overlay_is_applied_on_modern_skins() {
        let mut s = skin(64, 64, [10, 10, 10, 0]);
        s.set(40, 8, [200, 0, 0, 255]);
        let f = face(&s, true).unwrap();
        assert_eq!(f.get(0, 0), [200, 0, 0, 255]);
        assert_eq!(f.get(1, 0), [10, 10, 10, 255]);
        assert_eq!(face(&s, false).unwrap().get(0, 0), [10, 10, 10, 255]);
    }

    #[test]
    fn opaque_legacy_hat_is_ignored() {
        let s = skin(64, 32, [5, 6, 7, 255]);
        let mut s2 = s.clone();
        s2.set(40, 8, [1, 1, 1, 255]);
        assert_eq!(face(&s2, true).unwrap().get(0, 0), [5, 6, 7, 255]);
    }

    #[test]
    fn rejects_odd_sizes() {
        assert!(face(&Image::new(32, 32), true).is_err());
    }
}
