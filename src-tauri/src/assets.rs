//! Chapter images, kept as files named by the hash of their bytes.

use std::fs;
use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};
use tauri::http::{header, Response, StatusCode};

use crate::error::{AppError, AppResult};

/// Same ceiling as Firebase Storage's `validImageUpload`, so a stored image can be published.
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// The type comes from the bytes, never from a name or header the caller supplies.
fn sniff(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(("png", "image/png"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else {
        None
    }
}

/// Stores the image and returns its file name, which is also its asset id.
pub fn save_image(dir: &Path, bytes: &[u8]) -> AppResult<String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::validation(
            "Image too large. Maximum size is 2 MB.",
        ));
    }
    let (extension, _) = sniff(bytes)
        .ok_or_else(|| AppError::validation("Unsupported format. Use JPEG, PNG, or WebP."))?;
    let hash: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let name = format!("{hash}.{extension}");
    let path = dir.join(&name);
    if !path.exists() {
        // Write then rename, so a crash never leaves a half-written image under a valid name.
        let partial = dir.join(format!("{name}.partial"));
        let mut file = fs::File::create(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&partial, &path)?;
    }
    Ok(name)
}

/// Only names this module produces are served, which rules out path traversal.
fn is_asset_name(name: &str) -> bool {
    let Some((hash, extension)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 64
        && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        && matches!(extension, "jpg" | "png" | "webp")
}

/// Answers a `ttasset://` request from the webview.
pub fn serve(dir: &Path, name: &str) -> Response<Vec<u8>> {
    let found = is_asset_name(name)
        .then(|| fs::read(dir.join(name)).ok())
        .flatten()
        .and_then(|bytes| sniff(&bytes).map(|(_, mime)| (bytes, mime)));
    let response = match found {
        Some((bytes, mime)) => Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            )
            .body(bytes),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new()),
    };
    response.expect("static headers are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    #[test]
    fn saves_once_per_content_and_serves_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let name = save_image(dir.path(), PNG).unwrap();
        assert_eq!(save_image(dir.path(), PNG).unwrap(), name);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);

        let response = serve(dir.path(), &name);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.body(), PNG);
    }

    #[test]
    fn refuses_unknown_types_oversized_files_and_foreign_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(save_image(dir.path(), b"<svg onload=alert(1)>").is_err());
        let mut huge = PNG.to_vec();
        huge.resize(MAX_IMAGE_BYTES + 1, 0);
        assert!(save_image(dir.path(), &huge).is_err());

        fs::write(dir.path().join("secret.txt"), "x").unwrap();
        for name in ["secret.txt", "../secret.txt", ""] {
            assert_eq!(serve(dir.path(), name).status(), StatusCode::NOT_FOUND);
        }
    }
}
