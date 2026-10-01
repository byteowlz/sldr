//! Intrinsic pixel size of an embedded picture, read from its header (PNG,
//! JPEG, GIF — the formats the writer embeds). Just enough to place a picture
//! without distorting it; nothing is decoded.

/// `(width, height)` in pixels, or `None` for an unknown or truncated header.
pub(crate) fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    png(bytes).or_else(|| gif(bytes)).or_else(|| jpeg(bytes)).filter(|&(w, h)| w > 0 && h > 0)
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?])))
}

fn png(b: &[u8]) -> Option<(u32, u32)> {
    // Signature, then the IHDR chunk: length, "IHDR", width, height.
    if !b.starts_with(b"\x89PNG\r\n\x1a\n") || b.get(12..16)? != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(b.get(20..24)?.try_into().ok()?);
    Some((w, h))
}

fn gif(b: &[u8]) -> Option<(u32, u32)> {
    if !b.starts_with(b"GIF8") {
        return None;
    }
    let w = u16::from_le_bytes(b.get(6..8)?.try_into().ok()?);
    let h = u16::from_le_bytes(b.get(8..10)?.try_into().ok()?);
    Some((u32::from(w), u32::from(h)))
}

fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
    if !b.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    loop {
        // Markers may be padded with extra 0xFF bytes.
        while *b.get(i)? == 0xFF && *b.get(i + 1)? == 0xFF {
            i += 1;
        }
        if *b.get(i)? != 0xFF {
            return None;
        }
        let marker = *b.get(i + 1)?;
        match marker {
            // Standalone markers carry no length.
            0x01 | 0xD0..=0xD7 => i += 2,
            // Start of frame (baseline, progressive, …) — not DHT/JPG/DAC.
            0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                return Some((be16(b, i + 7)?, be16(b, i + 5)?));
            }
            0xD9 | 0xDA => return None, // end of image / scan before any frame
            _ => i += 2 + be16(b, i + 2)? as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_gif_and_jpeg_headers() {
        let mut png_bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png_bytes.extend_from_slice(&1160u32.to_be_bytes());
        png_bytes.extend_from_slice(&1150u32.to_be_bytes());
        assert_eq!(dimensions(&png_bytes), Some((1160, 1150)));

        let gif_bytes = [b"GIF89a".as_slice(), &640u16.to_le_bytes(), &480u16.to_le_bytes()].concat();
        assert_eq!(dimensions(&gif_bytes), Some((640, 480)));

        // SOI, an APP0 segment (length 16), then SOF0 with height 300, width 2000.
        let mut jpg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        jpg.extend_from_slice(&[0; 14]);
        jpg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        jpg.extend_from_slice(&300u16.to_be_bytes());
        jpg.extend_from_slice(&2000u16.to_be_bytes());
        assert_eq!(dimensions(&jpg), Some((2000, 300)));
    }

    #[test]
    fn unknown_or_truncated_headers_are_none() {
        assert_eq!(dimensions(b"not an image"), None);
        assert_eq!(dimensions(b"\x89PNG\r\n\x1a\n\0\0"), None);
        assert_eq!(dimensions(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]), None);
    }
}
