pub struct UnknownViewer;

impl UnknownViewer {
    pub fn label(type_id: u16) -> String {
        format!("Unknown Viewer (type: {type_id:#06x})")
    }
}
