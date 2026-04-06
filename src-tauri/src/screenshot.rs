use base64::{engine::general_purpose, Engine as _};
use image::DynamicImage;
use screenshots::Screen;
use std::io::Cursor;
use tracing::{info, warn};

/// Captures the primary screen and returns a base64-encoded JPEG data URL.
/// Returns None if capture fails (e.g., Screen Recording permission denied).
pub fn capture_primary_screen() -> Option<String> {
    let screens = Screen::all()
        .map_err(|e| warn!("Cannot get screen list: {}", e))
        .ok()?;

    let screen = screens
        .iter()
        .find(|s| s.display_info.is_primary)
        .or_else(|| screens.first())?;

    let image = screen
        .capture()
        .map_err(|e| warn!("Screenshot failed (may lack Screen Recording permission): {}", e))
        .ok()?;

    let width = image.width();
    let height = image.height();

    // screenshots 0.8 returns image::ImageBuffer<Rgba<u8>, Vec<u8>> directly.
    let dynamic = DynamicImage::ImageRgba8(image);

    // Resize to max 1280px on the longest side to keep payload reasonable.
    let max_dim = 1280u32;
    let resized = if width > max_dim || height > max_dim {
        dynamic.resize(max_dim, max_dim, image::imageops::FilterType::Triangle)
    } else {
        dynamic
    };

    let mut buf = Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| warn!("JPEG encoding failed: {}", e))
        .ok()?;

    let jpeg_bytes = buf.into_inner();
    info!(
        "Screenshot done: {}x{} → {} bytes JPEG",
        width,
        height,
        jpeg_bytes.len()
    );

    let b64 = general_purpose::STANDARD.encode(&jpeg_bytes);
    Some(format!("data:image/jpeg;base64,{}", b64))
}
