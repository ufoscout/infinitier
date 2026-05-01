use eframe::egui;

pub struct LuaViewer;

impl LuaViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("LUA Viewer");
    }
}
