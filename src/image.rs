//! Minimal RGBA image type: PNG decode/encode, crop, alpha compositing, scaling.

use std::io::Cursor;

pub type Rgba = [u8; 4];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub px: Vec<Rgba>,
}

impl Image {
    pub fn new(w: u32, h: u32) -> Image {
        Image {
            w,
            h,
            px: vec![[0; 4]; (w * h) as usize],
        }
    }

    pub fn from_rgba(w: u32, h: u32, bytes: &[u8]) -> Image {
        assert_eq!(bytes.len(), (w * h * 4) as usize);
        let px = bytes.as_chunks::<4>().0.to_vec();
        Image { w, h, px }
    }

    pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(
            png::Transformations::EXPAND
                | png::Transformations::STRIP_16
                | png::Transformations::ALPHA,
        );
        let mut reader = decoder
            .read_info()
            .map_err(|e| format!("invalid PNG: {e}"))?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| format!("invalid PNG: {e}"))?;
        let data = &buf[..info.buffer_size()];
        let px: Vec<Rgba> = match info.color_type {
            png::ColorType::Rgba => data.as_chunks::<4>().0.to_vec(),
            png::ColorType::Rgb => data
                .as_chunks::<3>()
                .0
                .iter()
                .map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[g, a]| [g, g, g, a])
                .collect(),
            png::ColorType::Grayscale => data.iter().map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => return Err("unexpected indexed PNG output".into()),
        };
        Ok(Image {
            w: info.width,
            h: info.height,
            px,
        })
    }

    pub fn encode_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.w, self.h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("PNG header");
            writer
                .write_image_data(&self.px.concat())
                .expect("PNG data");
        }
        out
    }

    pub fn get(&self, x: u32, y: u32) -> Rgba {
        self.px[(y * self.w + x) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, c: Rgba) {
        self.px[(y * self.w + x) as usize] = c;
    }

    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Result<Image, String> {
        if x + w > self.w || y + h > self.h {
            return Err(format!(
                "crop {w}x{h}+{x}+{y} is outside the {}x{} image",
                self.w, self.h
            ));
        }
        let mut out = Image::new(w, h);
        for dy in 0..h {
            for dx in 0..w {
                out.set(dx, dy, self.get(x + dx, y + dy));
            }
        }
        Ok(out)
    }

    /// Composites `src` over `self` at (dx, dy) using straight-alpha "over".
    pub fn over(&mut self, src: &Image, dx: i32, dy: i32) {
        for sy in 0..src.h {
            for sx in 0..src.w {
                let (tx, ty) = (dx + sx as i32, dy + sy as i32);
                if tx < 0 || ty < 0 || tx >= self.w as i32 || ty >= self.h as i32 {
                    continue;
                }
                let blended = blend(self.get(tx as u32, ty as u32), src.get(sx, sy));
                self.set(tx as u32, ty as u32, blended);
            }
        }
    }

    /// Nearest-neighbour upscale by an integer factor.
    pub fn scale(&self, k: u32) -> Image {
        if k == 1 {
            return self.clone();
        }
        let mut out = Image::new(self.w * k, self.h * k);
        for y in 0..out.h {
            for x in 0..out.w {
                out.set(x, y, self.get(x / k, y / k));
            }
        }
        out
    }
}

fn blend(dst: Rgba, src: Rgba) -> Rgba {
    let sa = src[3] as u32;
    if sa == 255 {
        return src;
    }
    if sa == 0 {
        return dst;
    }
    let da = dst[3] as u32;
    // out_a = sa + da * (1 - sa), in 0..=255*255 space.
    let out_a = sa * 255 + da * (255 - sa);
    if out_a == 0 {
        return [0; 4];
    }
    let mut out = [0u8; 4];
    for i in 0..3 {
        let c = src[i] as u32 * sa * 255 + dst[i] as u32 * da * (255 - sa);
        out[i] = (c / out_a) as u8;
    }
    out[3] = (out_a / 255) as u8;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_roundtrip() {
        let mut img = Image::new(3, 2);
        img.set(0, 0, [255, 0, 0, 255]);
        img.set(2, 1, [1, 2, 3, 128]);
        assert_eq!(Image::decode_png(&img.encode_png()).unwrap(), img);
    }

    #[test]
    fn scale_repeats_pixels() {
        let mut img = Image::new(2, 1);
        img.set(1, 0, [9, 9, 9, 255]);
        let s = img.scale(3);
        assert_eq!((s.w, s.h), (6, 3));
        assert_eq!(s.get(2, 2), [0; 4]);
        assert_eq!(s.get(3, 0), [9, 9, 9, 255]);
    }

    #[test]
    fn over_respects_alpha() {
        let mut base = Image::new(1, 1);
        base.set(0, 0, [0, 0, 0, 255]);
        let mut top = Image::new(1, 1);
        top.set(0, 0, [255, 255, 255, 0]);
        base.over(&top, 0, 0);
        assert_eq!(base.get(0, 0), [0, 0, 0, 255]);
        top.set(0, 0, [200, 100, 50, 255]);
        base.over(&top, 0, 0);
        assert_eq!(base.get(0, 0), [200, 100, 50, 255]);
    }
}
