//! `luma-studio [fichier RAW | dossier]` : ouvre LÜMA Studio (sur iPhone et iPad, le
//! dossier Documents de l'app).

use luma_studio::Studio;

struct App(Studio);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }

    fn on_exit(&mut self) {
        // Les derniers réglages ne sont pas perdus.
        self.0.flush_xmp();
    }
}

fn main() -> eframe::Result {
    let arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    // Sur iPhone et iPad, l'app s'ouvre sur son dossier Documents.
    let arg = arg.or_else(|| {
        cfg!(target_os = "ios")
            .then(luma_studio::dialogs::documents)
            .flatten()
    });
    let viewport = egui::ViewportBuilder::default()
        .with_title("LÜMA")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1024.0, 640.0])
        .with_drag_and_drop(true);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "LÜMA",
        options,
        Box::new(move |cc| {
            let mut studio = Studio::new(&cc.egui_ctx);
            if let Some(path) = &arg {
                studio.open_path(path);
            }
            Ok(Box::new(App(studio)))
        }),
    )
}
