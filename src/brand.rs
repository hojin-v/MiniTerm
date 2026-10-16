#[path = "../assets/brand_pixels.rs"]
mod pixels;
pub use pixels::rgba;
pub fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: rgba(64),
        width: 64,
        height: 64,
    }
}
