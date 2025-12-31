

/// Represents an RGB color with an alpha channel
#[derive(Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Alpha channel
    /// 0 - transparent
    /// 255 - opaque
    pub alpha: u8,
}
