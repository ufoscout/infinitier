/// Represents one MOS V1 palette entry. The on-disk layout is BGRA but
/// the alpha channel is conventionally ignored — NearInfinity always
/// renders entries as opaque (forces `alpha = 0xFF`) and treats the green
/// color `(0, 255, 0)` as transparent only when its "transparency
/// enabled" rendering mode is on.
///
/// The importer preserves the file's raw bytes here; transparency
/// handling is left to the renderer.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Alpha byte from the file. Almost always `0x00` in vanilla assets
    /// because the engine ignores it.
    pub alpha: u8,
}
