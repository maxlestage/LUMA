//! `luma-studio [fichier RAW | dossier]` : ouvre LÜMA Studio (sur iPhone et iPad, le
//! dossier Documents de l'app).
// Sous Windows, une app à fenêtre : pas de console noire à côté de LÜMA.
#![cfg_attr(windows, windows_subsystem = "windows")]

use luma_studio::Studio;

struct App(Studio, Frames);

/// Sur iPhone et iPad, toutes les cinq secondes où l'écran s'est redessiné, une ligne du
/// journal : combien d'images, la plus lente (le temps du processeur pour la dessiner),
/// et ce qui a demandé les redessins (le fichier et la ligne du code). De quoi savoir,
/// depuis l'appareil lui‑même, ce qui ralentit ou ce qui use la batterie.
struct Frames {
    since: std::time::Instant,
    count: u32,
    slowest: f32,
    causes: std::collections::BTreeMap<String, u32>,
    /// Les lignes écrites (le journal ne grossit pas sans fin).
    lines: u32,
}

impl Frames {
    fn new() -> Frames {
        Frames {
            since: std::time::Instant::now(),
            count: 0,
            slowest: 0.0,
            causes: Default::default(),
            lines: 0,
        }
    }

    fn record(&mut self, ctx: &egui::Context, cpu: Option<f32>) {
        if !cfg!(target_os = "ios") || self.lines >= 400 {
            return;
        }
        self.count += 1;
        self.slowest = self.slowest.max(cpu.unwrap_or(0.0) * 1000.0);
        for c in ctx.repaint_causes() {
            let file = c.file.rsplit('/').next().unwrap_or(c.file);
            *self.causes.entry(format!("{file}:{}", c.line)).or_default() += 1;
        }
        if self.since.elapsed().as_secs_f32() < 5.0 {
            return;
        }
        let mut causes: Vec<(String, u32)> = std::mem::take(&mut self.causes).into_iter().collect();
        causes.sort_by_key(|c| std::cmp::Reverse(c.1));
        let causes: Vec<String> = causes
            .iter()
            .take(4)
            .map(|(c, n)| format!("{c} ×{n}"))
            .collect();
        luma_studio::journal(&format!(
            "images : {} en {:.0} s, la plus lente {:.0} ms ; redessins demandés par {}",
            self.count,
            self.since.elapsed().as_secs_f32(),
            self.slowest,
            if causes.is_empty() {
                "l'utilisateur".to_owned()
            } else {
                causes.join(", ")
            }
        ));
        self.lines += 1;
        self.since = std::time::Instant::now();
        self.count = 0;
        self.slowest = 0.0;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.0.ui(ui);
        self.1.record(ui.ctx(), frame.info().cpu_usage);
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
    luma_studio::journal("démarrage");
    // La langue : LUMA_LANG, le choix fait dans l'app, ou celle du système.
    let lang = luma_studio::i18n::startup();
    luma_studio::i18n::set_lang(lang);
    luma_studio::journal(&format!("langue : {}", lang.code()));
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
            .with_min_inner_size([1024.0, 640.0])
            // Sous Linux, le nom qui relie la fenêtre à son lanceur (luma-studio.desktop) :
            // l'icône et le nom de LÜMA dans le dock et le sélecteur de fenêtres.
            .with_app_id("luma-studio");
    }
    // Le GPU : on demande ce que l'adaptateur offre vraiment, pas les limites par
    // défaut de wgpu (que le Metal du simulateur iOS, par exemple, n'atteint pas).
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.device_descriptor = std::sync::Arc::new(|adapter| {
        luma_studio::journal(&format!(
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
            luma_studio::journal("interface créée");
            // Sur iPhone et iPad : la vue de LÜMA, d'où s'ouvre le sélecteur de Photos.
            #[cfg(target_os = "ios")]
            {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                match cc.window_handle().map(|h| h.as_raw()) {
                    Ok(RawWindowHandle::UiKit(h)) => {
                        luma_apple::photos::set_view(h.ui_view.as_ptr() as usize);
                    }
                    other => luma_studio::journal(&format!("vue introuvable : {other:?}")),
                }
            }
            let mut studio = Studio::new(&cc.egui_ctx);
            if let Some(path) = &arg {
                studio.open_path(path);
            }
            Ok(Box::new(App(studio, Frames::new())))
        }),
    );
    luma_studio::journal(&format!("fin : {result:?}"));
    result
}
