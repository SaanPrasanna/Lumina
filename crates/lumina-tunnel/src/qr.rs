//! QR codes for public URLs, so a phone can open a shared site by scanning it.

use qrcode::QrCode;
use qrcode::render::svg;

use crate::{Error, Result};

/// `text` as a standalone SVG image (black on white, with a quiet zone).
pub fn svg(text: &str) -> Result<String> {
    let code = QrCode::new(text.as_bytes()).map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .quiet_zone(true)
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_an_svg() {
        let image = svg("https://abc123.ngrok-free.app").expect("qr");
        assert!(image.starts_with("<?xml"), "{image}");
        assert!(image.contains("<svg"));
        assert!(image.contains("#000000"));
    }
}
