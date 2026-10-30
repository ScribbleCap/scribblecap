use super::{EditableError, MAX_PAYLOAD_BYTES, payload};
use crate::{RasterFormat, RasterImage};

const PNG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PNG_CHUNK: &[u8; 4] = b"scED";
const WEBP_CHUNK: &[u8; 4] = b"SCAP";
const JPEG_FOOTER: &[u8; 8] = b"SCAPEDIT";

pub(super) struct Extracted<'a> {
    pub payload: Option<&'a [u8]>,
    pub dimensions: Option<(u32, u32)>,
    pub fingerprint: u32,
}

pub(super) fn prepare(
    mut flat: Vec<u8>,
    format: RasterFormat,
    preview: &RasterImage,
) -> Result<Vec<u8>, EditableError> {
    if format == RasterFormat::WebP {
        let extended = flat.get(12..16) == Some(b"VP8X");
        let mut out = Vec::new();
        out.try_reserve(flat.len() + 18)
            .map_err(|_| EditableError::Limits)?;
        out.extend_from_slice(&flat[..12]);
        if !extended {
            let (w, h) = (preview.width() - 1, preview.height() - 1);
            if w > 0xff_ffff || h > 0xff_ffff {
                return Err(EditableError::Limits);
            }
            out.extend_from_slice(b"VP8X\x0a\0\0\0");
            // Match the VP8L "alpha used" bit, even for an opaque image encoded
            // with an alpha channel. Pixel opacity alone does not describe it.
            let alpha = flat.get(12..16) == Some(b"VP8L")
                && flat.get(24).is_some_and(|byte| byte & 0x10 != 0);
            out.extend_from_slice(&[if alpha { 0x10 } else { 0 }, 0, 0, 0]);
            out.extend_from_slice(&w.to_le_bytes()[..3]);
            out.extend_from_slice(&h.to_le_bytes()[..3]);
        }
        out.extend_from_slice(&flat[12..]);
        let size = u32::try_from(out.len() - 8).map_err(|_| EditableError::Limits)?;
        out[4..8].copy_from_slice(&size.to_le_bytes());
        flat = out;
    }
    Ok(flat)
}

pub(super) fn embed(
    mut flat: Vec<u8>,
    format: RasterFormat,
    payload: &[u8],
) -> Result<Vec<u8>, EditableError> {
    match format {
        RasterFormat::Png => {
            if flat.len() < 12 || &flat[flat.len() - 8..flat.len() - 4] != b"IEND" {
                return Err(invalid("PNG has no terminal IEND"));
            }
            let end = flat.split_off(flat.len() - 12);
            flat.try_reserve(payload.len() + 24)
                .map_err(|_| EditableError::Limits)?;
            flat.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            flat.extend_from_slice(PNG_CHUNK);
            flat.extend_from_slice(payload);
            let mut crc = crc32fast::Hasher::new();
            crc.update(PNG_CHUNK);
            crc.update(payload);
            flat.extend_from_slice(&crc.finalize().to_be_bytes());
            flat.extend_from_slice(&end);
        }
        RasterFormat::Jpeg => {
            if !flat.ends_with(&[0xff, 0xd9]) {
                return Err(invalid("JPEG has no terminal EOI"));
            }
            flat.try_reserve(payload.len() + 16)
                .map_err(|_| EditableError::Limits)?;
            flat.extend_from_slice(payload);
            flat.extend_from_slice(&(payload.len() as u64).to_be_bytes());
            flat.extend_from_slice(JPEG_FOOTER);
        }
        RasterFormat::WebP => {
            let mut out = flat;
            out.try_reserve(8 + payload.len() + 1)
                .map_err(|_| EditableError::Limits)?;
            out.extend_from_slice(WEBP_CHUNK);
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
            if !payload.len().is_multiple_of(2) {
                out.push(0);
            }
            let size = u32::try_from(out.len() - 8).map_err(|_| EditableError::Limits)?;
            out[4..8].copy_from_slice(&size.to_le_bytes());
            flat = out;
        }
        _ => return Err(EditableError::UnsupportedFormat),
    }
    Ok(flat)
}

pub(super) fn extract(bytes: &[u8]) -> Result<Extracted<'_>, EditableError> {
    if bytes.starts_with(PNG) {
        return png(bytes);
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        return jpeg(bytes);
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return webp(bytes);
    }
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Bmp | image::ImageFormat::Tiff | image::ImageFormat::Gif) => {
            Ok(Extracted {
                payload: None,
                dimensions: None,
                fingerprint: 0,
            })
        }
        _ => Err(EditableError::UnsupportedFormat),
    }
}

fn png(bytes: &[u8]) -> Result<Extracted<'_>, EditableError> {
    let mut position = 8usize;
    let mut dimensions = None;
    let mut payload = None;
    let mut data_seen = false;
    let mut data_ended = false;
    let mut fingerprint = crc32fast::Hasher::new();
    fingerprint.update(PNG);
    while position < bytes.len() {
        let header = slice(bytes, position, 8)?;
        let length =
            u32::from_be_bytes(header[..4].try_into().map_err(|_| invalid("PNG length"))?) as usize;
        if length > 0x7fff_ffff {
            return Err(invalid("PNG chunk length exceeds specification"));
        }
        let kind = &header[4..8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err(invalid("PNG chunk type"));
        }
        let data = slice(bytes, position + 8, length)?;
        let end = position
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| invalid("PNG chunk length overflow"))?;
        let crc = slice(bytes, end - 4, 4)?;
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(kind);
        hasher.update(data);
        if hasher.finalize().to_be_bytes() != crc {
            return Err(invalid("PNG checksum mismatch"));
        }
        if position == 8 && (kind != b"IHDR" || length != 13) {
            return Err(invalid("PNG must begin with IHDR"));
        }
        if kind != PNG_CHUNK {
            fingerprint.update(&bytes[position..end]);
        }
        match kind {
            b"IHDR" => {
                if dimensions.is_some() || length != 13 {
                    return Err(invalid("duplicate or invalid IHDR"));
                }
                let w = u32::from_be_bytes(data[..4].try_into().map_err(|_| invalid("PNG width"))?);
                let h =
                    u32::from_be_bytes(data[4..8].try_into().map_err(|_| invalid("PNG height"))?);
                if w == 0 || h == 0 {
                    return Err(invalid("zero PNG dimensions"));
                }
                dimensions = Some((w, h));
            }
            b"IDAT" => {
                if data_ended {
                    return Err(invalid("nonconsecutive PNG image data"));
                }
                data_seen = true;
            }
            b"IEND" => {
                if length != 0 || end != bytes.len() || !data_seen {
                    return Err(invalid("invalid PNG end or trailing bytes"));
                }
                return Ok(Extracted {
                    payload,
                    dimensions,
                    fingerprint: fingerprint.finalize(),
                });
            }
            _ => {
                if data_seen {
                    data_ended = true;
                }
                if kind == PNG_CHUNK {
                    if !data_seen || payload.replace(data).is_some() {
                        return Err(invalid("misplaced or duplicate editable PNG chunk"));
                    }
                    if length > MAX_PAYLOAD_BYTES {
                        return Err(EditableError::Limits);
                    }
                }
            }
        }
        position = end;
    }
    Err(invalid("PNG has no IEND"))
}

fn jpeg(bytes: &[u8]) -> Result<Extracted<'_>, EditableError> {
    let mut position = 2usize;
    let mut scan = false;
    let mut dimensions = None;
    while position < bytes.len() {
        if scan {
            // Entropy data uses FF00 stuffing and restart markers. APP segment
            // contents are skipped by length, so thumbnail EOI bytes cannot be
            // mistaken for the end of this image.
            loop {
                let byte = *bytes
                    .get(position)
                    .ok_or_else(|| invalid("truncated JPEG scan"))?;
                if byte != 0xff {
                    position += 1;
                    continue;
                }
                let start = position;
                while bytes.get(position) == Some(&0xff) {
                    position += 1;
                }
                let marker = *bytes
                    .get(position)
                    .ok_or_else(|| invalid("truncated JPEG marker"))?;
                if marker == 0 || (0xd0..=0xd7).contains(&marker) {
                    position += 1;
                    continue;
                }
                position = start;
                break;
            }
            scan = false;
        }
        if bytes.get(position) != Some(&0xff) {
            return Err(invalid("invalid JPEG marker boundary"));
        }
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        let marker = *bytes
            .get(position)
            .ok_or_else(|| invalid("truncated JPEG marker"))?;
        position += 1;
        match marker {
            0xd9 => {
                let tail = &bytes[position..];
                let recognized = tail.starts_with(payload::MAGIC)
                    || (!tail.is_empty() && payload::MAGIC.starts_with(tail))
                    || tail.ends_with(JPEG_FOOTER);
                if !recognized {
                    return Ok(Extracted {
                        payload: None,
                        dimensions,
                        fingerprint: crc32fast::hash(&bytes[..position]),
                    });
                }
                if tail.len() < 16 || !tail.ends_with(JPEG_FOOTER) {
                    return Err(invalid("damaged editable JPEG footer"));
                }
                let at = tail.len() - 16;
                let length = u64::from_be_bytes(
                    tail[at..at + 8]
                        .try_into()
                        .map_err(|_| invalid("JPEG footer length"))?,
                );
                if length != at as u64 {
                    return Err(invalid("editable JPEG trailer length mismatch"));
                }
                if at > MAX_PAYLOAD_BYTES {
                    return Err(EditableError::Limits);
                }
                return Ok(Extracted {
                    payload: Some(&tail[..at]),
                    dimensions,
                    fingerprint: crc32fast::hash(&bytes[..position]),
                });
            }
            0x01 => continue,
            0x00 | 0xd0..=0xd8 => return Err(invalid("unexpected JPEG standalone marker")),
            _ => {}
        }
        let header = slice(bytes, position, 2)?;
        let length = u16::from_be_bytes([header[0], header[1]]) as usize;
        if length < 2 {
            return Err(invalid("invalid JPEG segment length"));
        }
        let segment = slice(bytes, position, length)?;
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if segment.len() < 8 {
                return Err(invalid("truncated JPEG frame header"));
            }
            dimensions = Some((
                u32::from(u16::from_be_bytes([segment[5], segment[6]])),
                u32::from(u16::from_be_bytes([segment[3], segment[4]])),
            ));
        }
        position += length;
        scan = marker == 0xda;
    }
    Err(invalid("JPEG has no EOI"))
}

fn webp(bytes: &[u8]) -> Result<Extracted<'_>, EditableError> {
    let size = slice(bytes, 4, 4)?;
    let size = u32::from_le_bytes(size.try_into().map_err(|_| invalid("WebP RIFF size"))?) as usize;
    if size.checked_add(8) != Some(bytes.len()) {
        return Err(invalid("WebP RIFF length mismatch"));
    }
    let mut position = 12usize;
    let mut payload = None;
    let mut dimensions = None;
    let mut image_seen = false;
    let mut body_hash = crc32fast::Hasher::new();
    let mut removed = 0;
    while position < bytes.len() {
        let header = slice(bytes, position, 8)?;
        let kind = &header[..4];
        let length = u32::from_le_bytes(
            header[4..8]
                .try_into()
                .map_err(|_| invalid("WebP chunk length"))?,
        ) as usize;
        let data = slice(bytes, position + 8, length)?;
        let end = position
            .checked_add(8)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| invalid("WebP chunk length overflow"))?;
        if !length.is_multiple_of(2) && bytes.get(end) != Some(&0) {
            return Err(invalid("missing/nonzero WebP padding"));
        }
        if kind == WEBP_CHUNK {
            removed = 8 + length + length % 2;
        } else {
            body_hash.update(slice(bytes, position, 8 + length + length % 2)?);
        }
        match kind {
            b"VP8X" => {
                if position != 12 || length != 10 || dimensions.is_some() {
                    return Err(invalid("invalid/duplicate WebP extended header"));
                }
                dimensions = Some((u24(&data[4..7]) + 1, u24(&data[7..10]) + 1));
            }
            b"VP8 " | b"VP8L" | b"ANMF" => image_seen = true,
            b"SCAP" => {
                if length > MAX_PAYLOAD_BYTES {
                    return Err(EditableError::Limits);
                }
                if payload.replace(data).is_some() {
                    return Err(invalid("duplicate editable WebP chunks"));
                }
            }
            _ => {}
        }
        position = end
            .checked_add(length % 2)
            .ok_or_else(|| invalid("WebP chunk overflow"))?;
    }
    if !image_seen || position != bytes.len() || (payload.is_some() && dimensions.is_none()) {
        return Err(invalid("incomplete WebP container"));
    }
    Ok(Extracted {
        payload,
        dimensions,
        fingerprint: {
            let size =
                u32::try_from(bytes.len() - removed - 8).map_err(|_| EditableError::Limits)?;
            let mut header = crc32fast::Hasher::new();
            header.update(b"RIFF");
            header.update(&size.to_le_bytes());
            header.update(b"WEBP");
            header.combine(&body_hash);
            header.finalize()
        },
    })
}

fn u24(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16
}

fn slice(bytes: &[u8], at: usize, length: usize) -> Result<&[u8], EditableError> {
    bytes
        .get(at..)
        .and_then(|rest| rest.get(..length))
        .ok_or_else(|| invalid("truncated chunk or forged length"))
}

fn invalid(reason: &'static str) -> EditableError {
    EditableError::InvalidContainer(reason)
}
