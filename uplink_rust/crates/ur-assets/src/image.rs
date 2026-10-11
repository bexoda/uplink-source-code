use std::io::Cursor;

use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;
use tiff::ColorType;

use crate::AssetError;

const MAX_PIXELS: u64 = 16 * 1024 * 1024;

/// 8-bit RGBA, row-major, straight (non-premultiplied) alpha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rgba {
    /// Uplink's sprites use pure black as "nothing". Turn black into transparency.
    pub fn black_to_alpha(mut self) -> Rgba {
        for px in self.pixels.chunks_exact_mut(4) {
            if px[0] == 0 && px[1] == 0 && px[2] == 0 {
                px[3] = 0;
            }
        }
        self
    }
}

pub fn decode_tiff(bytes: &[u8]) -> Result<Rgba, AssetError> {
    let bad = |e: tiff::TiffError| AssetError::BadImage(e.to_string());
    let mut d = Decoder::new(Cursor::new(bytes)).map_err(bad)?;
    let (width, height) = d.dimensions().map_err(bad)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(AssetError::BadImage(format!(
            "unreasonable size {width}x{height}"
        )));
    }
    // 8-bit palette images (most of the loading screens): the `tiff` crate refuses the colour
    // type before decoding, so read uncompressed ones directly.
    let photometric = d
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .map_err(bad)?;
    let palette8 = photometric == Some(3)
        && d.find_tag_unsigned::<u16>(Tag::BitsPerSample)
            .ok()
            .flatten()
            == Some(8);
    if palette8 {
        return decode_palette8(&mut d, bytes, width, height);
    }
    let color = d.colortype().map_err(bad)?;
    if matches!(color, ColorType::Gray(1)) {
        return decode_bilevel(&mut d, bytes, width, height);
    }
    let palette = if matches!(color, ColorType::Palette(8)) {
        Some(d.get_tag_u16_vec(Tag::ColorMap).map_err(bad)?)
    } else {
        None
    };
    let DecodingResult::U8(data) = d.read_image().map_err(bad)? else {
        return Err(AssetError::BadImage(
            "only 8-bit images are supported".into(),
        ));
    };
    let n = (width * height) as usize;
    let mut pixels = Vec::with_capacity(n * 4);
    let short = || AssetError::BadImage("pixel data shorter than the header claims".into());
    match color {
        ColorType::RGBA(8) => {
            if data.len() < n * 4 {
                return Err(short());
            }
            pixels.extend_from_slice(&data[..n * 4]);
        }
        ColorType::RGB(8) => {
            if data.len() < n * 3 {
                return Err(short());
            }
            for c in data[..n * 3].chunks_exact(3) {
                pixels.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        ColorType::Gray(8) => {
            if data.len() < n {
                return Err(short());
            }
            for &g in &data[..n] {
                pixels.extend_from_slice(&[g, g, g, 255]);
            }
        }
        ColorType::Palette(8) => {
            let map = palette.unwrap_or_default();
            let entries = map.len() / 3;
            if data.len() < n {
                return Err(short());
            }
            for &i in &data[..n] {
                let i = i as usize;
                if i >= entries {
                    return Err(AssetError::BadImage("palette index out of range".into()));
                }
                pixels.extend_from_slice(&[
                    (map[i] >> 8) as u8,
                    (map[entries + i] >> 8) as u8,
                    (map[2 * entries + i] >> 8) as u8,
                    255,
                ]);
            }
        }
        other => {
            return Err(AssetError::BadImage(format!(
                "unsupported colour type {other:?}"
            )))
        }
    }
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}

/// Uncompressed 8-bit palette images, read strip by strip.
fn decode_palette8(
    d: &mut Decoder<Cursor<&[u8]>>,
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<Rgba, AssetError> {
    let bad = |e: tiff::TiffError| AssetError::BadImage(e.to_string());
    if d.find_tag_unsigned::<u32>(Tag::Compression)
        .map_err(bad)?
        .unwrap_or(1)
        != 1
    {
        return Err(AssetError::BadImage(
            "compressed palette images are not supported".into(),
        ));
    }
    let map = d.get_tag_u16_vec(Tag::ColorMap).map_err(bad)?;
    let entries = map.len() / 3;
    let offsets = d.get_tag_u32_vec(Tag::StripOffsets).map_err(bad)?;
    let counts = d.get_tag_u32_vec(Tag::StripByteCounts).map_err(bad)?;
    let n = (width * height) as usize;
    let mut idx = Vec::with_capacity(n);
    for (&off, &cnt) in offsets.iter().zip(&counts) {
        let strip = bytes
            .get(off as usize..off as usize + cnt as usize)
            .ok_or_else(|| AssetError::BadImage("strip outside the file".into()))?;
        idx.extend_from_slice(strip);
        if idx.len() >= n {
            break;
        }
    }
    if idx.len() < n {
        return Err(AssetError::BadImage(
            "pixel data shorter than the header claims".into(),
        ));
    }
    let mut pixels = Vec::with_capacity(n * 4);
    for &i in &idx[..n] {
        let i = i as usize;
        if i >= entries {
            return Err(AssetError::BadImage("palette index out of range".into()));
        }
        pixels.extend_from_slice(&[
            (map[i] >> 8) as u8,
            (map[entries + i] >> 8) as u8,
            (map[2 * entries + i] >> 8) as u8,
            255,
        ]);
    }
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}

/// 1-bit uncompressed images (Uplink's land masks). Packed MSB-first, rows padded to whole bytes,
/// stored in strips. The `tiff` crate can't decode these, so read the strips directly.
fn decode_bilevel(
    d: &mut Decoder<Cursor<&[u8]>>,
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<Rgba, AssetError> {
    let bad = |e: tiff::TiffError| AssetError::BadImage(e.to_string());
    if d.find_tag_unsigned::<u32>(Tag::Compression)
        .map_err(bad)?
        .unwrap_or(1)
        != 1
    {
        return Err(AssetError::BadImage(
            "compressed 1-bit images are not supported".into(),
        ));
    }
    let offsets = d.get_tag_u32_vec(Tag::StripOffsets).map_err(bad)?;
    let counts = d.get_tag_u32_vec(Tag::StripByteCounts).map_err(bad)?;
    let rows_per_strip = d
        .find_tag_unsigned::<u32>(Tag::RowsPerStrip)
        .map_err(bad)?
        .unwrap_or(height)
        .max(1);
    // Photometric 0 = WhiteIsZero (set bit is black), 1 = BlackIsZero (set bit is white).
    let white_is_zero = d
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .map_err(bad)?
        == Some(0);
    let row_bytes = (width as usize).div_ceil(8);
    let mut pixels = Vec::with_capacity((width * height) as usize * 4);
    let mut row = 0u32;
    for (&off, &cnt) in offsets.iter().zip(&counts) {
        let strip = bytes
            .get(off as usize..off as usize + cnt as usize)
            .ok_or_else(|| AssetError::BadImage("strip outside the file".into()))?;
        for r in 0..rows_per_strip.min(height - row) {
            let line = strip
                .get(r as usize * row_bytes..(r as usize + 1) * row_bytes)
                .ok_or_else(|| AssetError::BadImage("strip too short".into()))?;
            for x in 0..width as usize {
                let set = line[x / 8] & (0x80 >> (x % 8)) != 0;
                let v = if set != white_is_zero { 255 } else { 0 };
                pixels.extend_from_slice(&[v, v, v, 255]);
            }
        }
        row += rows_per_strip.min(height - row);
        if row >= height {
            break;
        }
    }
    if row < height {
        return Err(AssetError::BadImage("fewer strips than rows".into()));
    }
    Ok(Rgba {
        width,
        height,
        pixels,
    })
}
