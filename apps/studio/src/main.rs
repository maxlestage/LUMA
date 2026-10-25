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
    // Sur iPhone et iPad, une erreur fatale est aussi écrite dans Documents (visible
    // dans l'app Fichiers) : il n'y a pas de terminal pour la lire.
    if cfg!(target_os = "ios")
        && let Some(dir) = luma_studio::dialogs::documents()
    {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let text = format!("{info}\n{}", std::backtrace::Backtrace::force_capture());
            let _ = std::fs::write(dir.join("LÜMA-erreur.txt"), text);
            default(info);
        }));
    }
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
