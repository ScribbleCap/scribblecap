//! Raw SCOP command packets: bounded JSON metadata followed by straight RGBA8.
use scribblecap_formats::RasterImage;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use tauri::ipc::{InvokeBody, Request};

const MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;
const MAX_PIXEL_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize)]
struct Metadata<T> {
    width: u32,
    height: u32,
    #[serde(flatten)]
    fields: T,
}

pub fn decode<T: DeserializeOwned>(request: &Request<'_>) -> Result<(RasterImage, T), String> {
    match request.body() {
        InvokeBody::Raw(bytes) => decode_bytes(bytes),
        InvokeBody::Json(_) => Err("Image commands require a raw binary request.".into()),
    }
}

fn decode_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<(RasterImage, T), String> {
    let invalid = || "The image request is invalid.".to_string();
    if bytes.len() < 12 || &bytes[..4] != b"SCOP" || bytes[4..8] != 1_u32.to_le_bytes() {
        return Err(invalid());
    }
    let metadata_len = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| invalid())?) as usize;
    if metadata_len > MAX_METADATA_BYTES || metadata_len > bytes.len() - 12 {
        return Err(invalid());
    }
    let offset = 12 + metadata_len;
    let metadata: Metadata<T> =
        serde_json::from_slice(&bytes[12..offset]).map_err(|_| invalid())?;
    let expected = u64::from(metadata.width)
        .checked_mul(u64::from(metadata.height))
        .and_then(|size| size.checked_mul(4))
        .ok_or_else(invalid)?;
    if expected == 0
        || expected > MAX_PIXEL_BYTES as u64
        || expected != (bytes.len() - offset) as u64
    {
        return Err(invalid());
    }
    let image = RasterImage::new(metadata.width, metadata.height, bytes[offset..].to_vec())
        .map_err(|_| invalid())?;
    Ok((image, metadata.fields))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, Deserialize)]
    struct Fields {
        name: String,
    }
    fn packet(metadata: &str, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = b"SCOP".to_vec();
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend((metadata.len() as u32).to_le_bytes());
        bytes.extend(metadata.as_bytes());
        bytes.extend(pixels);
        bytes
    }
    #[test]
    fn decodes_utf8_metadata_and_raw_rgba() {
        let bytes = packet(
            r#"{"width":1,"height":1,"name":"雪.png"}"#,
            &[0, 128, 255, 4],
        );
        let (image, fields) = decode_bytes::<Fields>(&bytes).unwrap();
        assert_eq!(image.pixels(), &[0, 128, 255, 4]);
        assert_eq!(fields.name, "雪.png");
    }
    #[test]
    fn rejects_bad_framing_metadata_and_pixel_lengths() {
        for bytes in [
            vec![],
            b"SCOP".to_vec(),
            packet(r#"{"width":1,"height":1,"name":"x"}"#, &[1, 2, 3]),
            packet(r#"{"width":1,"height":1,"name":"x"}"#, &[1; 5]),
            packet(r#"{"width":0,"height":1,"name":"x"}"#, &[]),
            packet(
                r#"{"width":4294967295,"height":4294967295,"name":"x"}"#,
                &[],
            ),
            packet(r#"{"width":1,"height":1}"#, &[1; 4]),
            packet("{", &[1; 4]),
        ] {
            assert!(decode_bytes::<Fields>(&bytes).is_err());
        }
        let valid = packet(r#"{"width":1,"height":1,"name":"x"}"#, &[1; 4]);
        for (index, value) in [(0, 0), (4, 2), (8, 255), (11, 255)] {
            let mut bytes = valid.clone();
            bytes[index] = value;
            assert!(decode_bytes::<Fields>(&bytes).is_err());
        }
    }
}
