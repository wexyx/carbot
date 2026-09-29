use super::{CompressionStrategy, disabled::Disabled, extractive::Extractive, window::Window};
pub struct CompressionFactory;
impl CompressionFactory {
    pub fn create(name: &str) -> Result<Box<dyn CompressionStrategy>, String> {
        match name {
            "intelligent" => Ok(Box::new(super::intelligent::Intelligent)),
            "extractive" => Ok(Box::new(Extractive)),
            "window" => Ok(Box::new(Window)),
            "disabled" => Ok(Box::new(Disabled)),
            _ => Err("CONTEXT_STRATEGY must be intelligent, extractive, window or disabled".into()),
        }
    }
}
