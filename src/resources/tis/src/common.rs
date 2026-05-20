/// One TIS palette entry. Stored on disk as BGRA; the alpha byte is
/// "unused" per the IESDP and is left untouched here. Rendering applies
/// transparency only when palette index 0 has RGB `(0, 255, 0)` — the
/// engine's green-screen convention; see
/// [`TisPalette::to_image`](crate::TisPalette::to_image).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Alpha byte from the file. Almost always `0x00` in vanilla assets
    /// because the engine ignores it.
    pub alpha: u8,
}
