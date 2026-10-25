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
    journal("démarrage");
    // La langue : LUMA_LANG, le choix fait dans l'app, ou celle du système.
    let lang = luma_studio::i18n::startup();
    luma_studio::i18n::set_lang(lang);
    journal(&format!("langue : {}", lang.code()));
    // HEIC, AVIF… : le décodeur d'images du système (ImageIO sur Mac, iPhone et iPad).
    luma_apple::install();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("LÜMA")
        .with_drag_and_drop(true);
    if !cfg!(target_os = "ios") {
        // L'icône de la fenêtre et du Dock (pendant le développement, hors du .app).
        let n = 256;
        viewport = viewport.with_icon(std::sync::Arc::new(egui::IconData {
            rgba: luma_brand::logo(n, luma_brand::Backdrop::MacIcon, true),
            width: n as u32,
            height: n as u32,
        }));
        viewport = viewport
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1024.0, 640.0]);
    }
    // Le GPU : on demande ce que l'adaptateur offre vraiment, pas les limites par
    // défaut de wgpu (que le Metal du simulateur iOS, par exemple, n'atteint pas).
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.device_descriptor = std::sync::Arc::new(|adapter| {
        journal(&format!(
            "GPU : {} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        ));
        wgpu::DeviceDescriptor {
            label: Some("LÜMA"),
            required_limits: adapter.limits(),
            ..Default::default()
        }
    });
    let options = eframe::NativeOptions {
        viewport,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(setup),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = eframe::run_native(
        "LÜMA",
        options,
        Box::new(move |cc| {
            journal("interface créée");
            let mut studio = Studio::new(&cc.egui_ctx);
            if let Some(path) = &arg {
                studio.open_path(path);
            }
            Ok(Box::new(App(studio)))
        }),
    );
    journal(&format!("fin : {result:?}"));
    result
}

/// Le journal de démarrage : sur la sortie d'erreur, et sur iPhone et iPad dans
/// `Documents/LÜMA-journal.txt` (lisible dans l'app Fichiers).
fn journal(message: &str) {
    eprintln!("LÜMA : {message}");
    if cfg!(target_os = "ios")
        && let Some(dir) = luma_studio::dialogs::documents()
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("LÜMA-journal.txt"))
        {
            let _ = writeln!(f, "{message}");
        }
    }
}
