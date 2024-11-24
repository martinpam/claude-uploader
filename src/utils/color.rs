use eframe::egui::Color32;

pub trait ColorExt {
    fn from_hex(hex: &str) -> Option<Self>
    where
        Self: Sized;
}

impl ColorExt for Color32 {
    fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

        Some(Color32::from_rgb(r, g, b))
    }
}
