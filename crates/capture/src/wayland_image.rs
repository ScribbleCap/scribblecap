//! File-URI handling and decoding, tested without a running Linux desktop.
use std::{io::Read, path::PathBuf};

use crate::{CaptureError, CapturedImage, Result};

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;

fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| CaptureError::Os(format!("reading portal screenshot: {error}")))?;
    if bytes.len() as u64 > limit {
        return Err(CaptureError::Os(
            "portal screenshot exceeds the file size limit".into(),
        ));
    }
    Ok(bytes)
}

fn screenshot_path(uri: &str) -> Result<PathBuf> {
    let url = url::Url::parse(uri)
        .map_err(|error| CaptureError::Os(format!("invalid portal screenshot URI: {error}")))?;
    if url.scheme() != "file"
        || url.host_str().is_some_and(|host| host != "localhost")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CaptureError::Os(
            "Screenshot portal must return a local file URI without query or fragment".into(),
        ));
    }
    url.to_file_path()
        .map_err(|()| CaptureError::Os("invalid local screenshot file URI".into()))
}

pub(crate) fn load(uri: &str) -> Result<CapturedImage> {
    let path = screenshot_path(uri)?;
    let file = std::fs::File::open(path)
        .map_err(|error| CaptureError::Os(format!("reading portal screenshot: {error}")))?;
    let bytes = read_limited(file, MAX_FILE_BYTES)?;
    // decode checks codec/output allocation limits before allocating pixel buffers;
    // this backend additionally rejects completed RGBA images above its lower budget.
    let image = scribblecap_formats::decode(&bytes)
        .map_err(|error| CaptureError::Os(format!("decoding portal screenshot: {error}")))?
        .image;
    if image.pixels().len() > MAX_IMAGE_BYTES {
        return Err(CaptureError::Os(
            "portal screenshot exceeds the 256 MiB RGBA image budget".into(),
        ));
    }
    Ok(CapturedImage {
        width: image.width(),
        height: image.height(),
        pixels: image.into_pixels(),
        // Screenshot provides no output bounds or scale metadata. Expose pixel
        // coordinates (scale 1), never guess a desktop-points mapping.
        scale: 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scribblecap_formats::{RasterFormat, RasterImage, encode};

    #[test]
    fn bounded_reads_allow_exact_limit_and_reject_larger_files() {
        assert_eq!(read_limited(&b"1234"[..], 4).unwrap(), b"1234");
        assert!(read_limited(&b"12345"[..], 4).is_err());
    }

    #[test]
    fn accepts_local_percent_encoded_file_uris() {
        let source = std::env::temp_dir().join("screenshot #é.png");
        let uri = url::Url::from_file_path(&source).unwrap();
        assert!(uri.as_str().contains("%20%23%C3%A9"));
        let path = screenshot_path(uri.as_str()).unwrap();
        assert_eq!(path.file_name().unwrap(), "screenshot #é.png");
        let local = uri.as_str().replacen("file:///", "file://localhost/", 1);
        assert!(screenshot_path(&local).is_ok());
    }

    #[test]
    fn rejects_remote_and_ambiguous_uris() {
        for uri in [
            "https://example.com/screen.png",
            "file://server/tmp/screen.png",
            "file:///tmp/screen.png?query",
            "file:///tmp/screen.png#fragment",
            "not a URI",
        ] {
            assert!(screenshot_path(uri).is_err(), "{uri}");
        }
    }

    #[test]
    fn decodes_rgba_and_preserves_the_portal_file() {
        let dir = std::env::temp_dir().join(format!("scribblecap-portal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("screen #é.png");
        let raster = RasterImage::new(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        std::fs::write(&path, encode(&raster, RasterFormat::Png).unwrap()).unwrap();
        let uri = url::Url::from_file_path(&path).unwrap();
        let image = load(uri.as_str()).unwrap();
        assert_eq!((image.width, image.height, image.scale), (2, 1, 1.0));
        assert_eq!(image.pixels, raster.pixels());
        assert!(path.exists());
        std::fs::write(&path, b"not an image").unwrap();
        assert!(load(uri.as_str()).is_err());
        std::fs::remove_file(&path).unwrap();
        assert!(load(uri.as_str()).is_err());
        std::fs::remove_dir(&dir).unwrap();
    }
}
