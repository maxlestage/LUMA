//! Importer des photos depuis l'app Photos, sur iPhone et iPad : le sélecteur du système
//! (PHPickerViewController, sans autorisation à demander : l'utilisateur ne donne que
//! ce qu'il choisit). Chaque photo choisie est copiée telle quelle (RAW, ProRAW, HEIC,
//! JPEG : le fichier d'origine, sans conversion) dans le dossier donné, puis le
//! compteur [`imported`] avance : à l'app de relire ce dossier.
//!
//! Ailleurs qu'iPhone et iPad, il n'y a pas de sélecteur de Photos : [`pick`] rend
//! `false`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static IMPORTED: AtomicUsize = AtomicUsize::new(0);
static CHOSEN: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);
static VIEW: AtomicUsize = AtomicUsize::new(0);

/// La vue de LÜMA (l'adresse de sa UIView, rendue par la fenêtre d'eframe) : le
/// sélecteur sera présenté par le contrôleur de sa fenêtre, plutôt que par une fenêtre
/// cherchée parmi celles de l'app (sur un appareil, il y en a d'autres : clavier,
/// TestFlight…, d'où rien ne s'affiche). À appeler une fois, sur le fil principal.
pub fn set_view(view: usize) {
    VIEW.store(view, Ordering::Release);
}

/// Le nombre de photos copiées depuis le lancement : quand il change, de nouvelles
/// photos sont dans le dossier d'import.
pub fn imported() -> usize {
    IMPORTED.load(Ordering::Acquire)
}

/// Le nombre de photos choisies dans le sélecteur depuis le lancement (copiées ou non).
pub fn chosen() -> usize {
    CHOSEN.load(Ordering::Acquire)
}

/// Le nombre de photos choisies qui n'ont pas pu être copiées.
pub fn failed() -> usize {
    FAILED.load(Ordering::Acquire)
}

/// Ouvre le sélecteur de Photos ; les photos choisies arrivent dans `dest`. À appeler sur
/// le fil principal. Rend le contrôleur qui l'a présenté (pour le journal) ; en cas
/// d'échec, ce qui a manqué (pour le dire à l'utilisateur).
pub fn pick(dest: PathBuf) -> Result<String, &'static str> {
    #[cfg(target_os = "ios")]
    {
        ios::pick(dest)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = dest;
        Err("pas de Photos ici")
    }
}

/// À appeler à chaque image tant qu'un import est en cours : si le sélecteur n'est
/// toujours pas à l'écran une demi‑seconde après la demande (iOS a refusé de le
/// présenter depuis la fenêtre de LÜMA), il est présenté depuis une fenêtre de
/// secours. Rend ce qui vient de se passer, pour le dire (une seule fois).
pub fn watch() -> Option<&'static str> {
    #[cfg(target_os = "ios")]
    {
        ios::watch()
    }
    #[cfg(not(target_os = "ios"))]
    None
}

/// Un sélecteur est‑il demandé et pas encore fermé ?
pub fn pending() -> bool {
    #[cfg(target_os = "ios")]
    {
        ios::pending()
    }
    #[cfg(not(target_os = "ios"))]
    false
}

/// Ouvre l'app Fichiers (une carte mémoire branchée, iCloud Drive, un disque) : les
/// photos choisies (RAW, HEIC, JPEG…) arrivent dans `dest`, comptées comme celles de
/// Photos ([`imported`], [`failed`]). À appeler sur le fil principal. En cas d'échec, ce
/// qui a manqué.
pub fn pick_files(dest: PathBuf) -> Result<(), &'static str> {
    #[cfg(target_os = "ios")]
    {
        ios::pick_files(dest)
    }
    #[cfg(not(target_os = "ios"))]
    {
        let _ = dest;
        Err("pas d'app Fichiers ici")
    }
}

/// Le sélecteur de Photos existe‑t‑il (iPhone et iPad) ?
pub const AVAILABLE: bool = cfg!(target_os = "ios");

static SAVED: AtomicUsize = AtomicUsize::new(0);
static SAVE_FAILED: AtomicUsize = AtomicUsize::new(0);

/// Ajoute une image exportée (JPEG, PNG, TIFF, HEIC) à la Pellicule de l'app Photos,
/// telle quelle : son EXIF, ses informations IPTC et son profil de couleur restent. La
/// première fois, iOS demande l'autorisation d'ajouter des photos. Le résultat arrive
/// plus tard : voir [`saved`] et [`save_failed`]. Ailleurs qu'iPhone et iPad, rien.
pub fn save_to_library(path: &std::path::Path) {
    #[cfg(target_os = "ios")]
    ios::save(path);
    #[cfg(not(target_os = "ios"))]
    let _ = path;
}

/// Le nombre d'images ajoutées à Photos depuis le lancement.
pub fn saved() -> usize {
    SAVED.load(Ordering::Acquire)
}

/// Le nombre d'images que Photos a refusées (autorisation refusée, format inconnu).
pub fn save_failed() -> usize {
    SAVE_FAILED.load(Ordering::Acquire)
}

/// Un nom libre dans `dir` pour `name` : `IMG_0001.DNG`, sinon `IMG_0001 2.DNG`…
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
fn free_name(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if !path.exists() {
        return path;
    }
    let p = std::path::Path::new(name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("photo");
    let ext = p.extension().and_then(|s| s.to_str());
    (2..)
        .map(|n| match ext {
            Some(e) => dir.join(format!("{stem} {n}.{e}")),
            None => dir.join(format!("{stem} {n}")),
        })
        .find(|p| !p.exists())
        .expect("un nom libre")
}

#[cfg(target_os = "ios")]
mod ios {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

    use block2::{Block, RcBlock};
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject, AnyProtocol, NSObject, NSObjectProtocol};
    use objc2::{ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_foundation::{NSArray, NSError, NSString, NSURL};
    use objc2_photos_ui::{
        PHPickerConfiguration, PHPickerConfigurationAssetRepresentationMode, PHPickerFilter, PHPickerResult,
    };
    use objc2_ui_kit::{
        UIApplication, UIDocumentPickerDelegate, UIDocumentPickerViewController, UIView, UIViewController, UIWindow,
        UIWindowScene,
    };
    use std::sync::atomic::AtomicBool;

    use super::{CHOSEN, FAILED, IMPORTED, SAVE_FAILED, SAVED, VIEW, free_name};

    struct Ivars {
        dest: PathBuf,
    }

    define_class!(
        // SAFETY: NSObject n'impose rien à ses sous‑classes ; `Delegate` n'implémente pas
        // `Drop` et ne vit que sur le fil principal.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LumaPhotoPickerDelegate"]
        #[ivars = Ivars]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        impl Delegate {
            // SAFETY: la signature de `PHPickerViewControllerDelegate` (PhotosUI) :
            // le sélecteur et les résultats choisis.
            #[unsafe(method(picker:didFinishPicking:))]
            fn did_finish(&self, picker: &AnyObject, results: &NSArray<PHPickerResult>) {
                // SAFETY: le sélecteur est un UIViewController présenté par l'app ; on le
                // ferme, sans bloc de fin.
                let _: () = unsafe {
                    msg_send![picker, dismissViewControllerAnimated: true, completion: None::<&Block<dyn Fn()>>]
                };
                PENDING.with(|p| *p.borrow_mut() = None);
                // La fenêtre de secours, s'il a fallu l'ouvrir, s'en va avec lui.
                PRESENTER.with(|w| {
                    if let Some(w) = w.borrow_mut().take() {
                        w.setHidden(true);
                    }
                });
                CHOSEN.fetch_add(results.count(), Ordering::Release);
                for result in results {
                    load(&result, self.ivars().dest.clone());
                }
            }
        }
    );

    define_class!(
        // SAFETY: NSObject n'impose rien à ses sous‑classes ; `FilesDelegate`
        // n'implémente pas `Drop` et ne vit que sur le fil principal.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LumaFilePickerDelegate"]
        #[ivars = Ivars]
        struct FilesDelegate;

        unsafe impl NSObjectProtocol for FilesDelegate {}

        // SAFETY: la signature du protocole (objc2-ui-kit).
        unsafe impl UIDocumentPickerDelegate for FilesDelegate {
            #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
            fn picked(&self, _picker: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
                CHOSEN.fetch_add(urls.count(), Ordering::Release);
                for url in urls {
                    let dest = &self.ivars().dest;
                    let taken = url
                        .path()
                        .zip(url.lastPathComponent())
                        .is_some_and(|(path, name)| take(path.to_string().as_ref(), dest, &name.to_string()));
                    let counter = if taken { &IMPORTED } else { &FAILED };
                    counter.fetch_add(1, Ordering::Release);
                }
            }
        }
    );

    /// Range dans `dest` une copie que l'app Fichiers vient de faire (dans le dossier
    /// temporaire de LÜMA) : déplacée, sinon copiée.
    fn take(path: &std::path::Path, dest: &std::path::Path, name: &str) -> bool {
        let target = free_name(dest, name);
        std::fs::create_dir_all(dest).is_ok()
            && (std::fs::rename(path, &target).is_ok() || std::fs::copy(path, &target).is_ok())
    }

    thread_local! {
        /// Le délégué du sélecteur en cours : le sélecteur ne le retient pas.
        static DELEGATE: RefCell<Option<Retained<Delegate>>> = const { RefCell::new(None) };
        /// Le délégué de l'app Fichiers ouverte, pour la même raison.
        static FILES_DELEGATE: RefCell<Option<Retained<FilesDelegate>>> = const { RefCell::new(None) };
        /// La fenêtre de secours qui présente le sélecteur quand la fenêtre de LÜMA ne
        /// le peut pas (voir `pick`).
        static PRESENTER: RefCell<Option<Retained<UIWindow>>> = const { RefCell::new(None) };
        /// Le sélecteur demandé et pas encore fermé : depuis quand, et s'il est déjà
        /// passé par la fenêtre de secours.
        static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
    }

    struct Pending {
        picker: Retained<AnyObject>,
        since: std::time::Instant,
        rescued: bool,
        /// « À l'écran » déjà dit.
        told: bool,
        top: Retained<UIViewController>,
    }

    /// Copie le fichier d'origine d'une photo choisie dans `dest`.
    fn load(result: &PHPickerResult, dest: PathBuf) {
        // SAFETY: lectures sur un résultat que le système vient de rendre.
        let provider = unsafe { result.itemProvider() };
        let raw = NSString::from_str("public.camera-raw-image");
        let kind = if provider.hasItemConformingToTypeIdentifier(&raw) {
            raw
        } else {
            NSString::from_str("public.image")
        };
        // Le système donne un fichier temporaire, effacé après ce bloc : on le copie ici.
        let copy = RcBlock::new(move |url: *mut NSURL, _error: *mut NSError| {
            // SAFETY: `url` est nul ou un NSURL valide pendant l'appel du bloc.
            let copied = (unsafe { url.as_ref() })
                .and_then(|url| Some((url.path()?, url.lastPathComponent()?)))
                .is_some_and(|(path, name)| {
                    let target = free_name(&dest, &name.to_string());
                    std::fs::create_dir_all(&dest).is_ok() && std::fs::copy(path.to_string(), &target).is_ok()
                });
            let counter = if copied { &IMPORTED } else { &FAILED };
            counter.fetch_add(1, Ordering::Release);
        });
        // SAFETY: le bloc ne capture que des valeurs `'static` et peut être appelé sur
        // n'importe quel fil ; le progrès rendu n'est pas utilisé.
        let _ = unsafe { provider.loadFileRepresentationForTypeIdentifier_completionHandler(&kind, &copy) };
    }

    /// Le contrôleur au premier plan, qui présentera le sélecteur : celui de la fenêtre
    /// de LÜMA (sa vue, donnée par `set_view`), sinon celui de la fenêtre principale
    /// cherchée dans les scènes et parmi les fenêtres de l'app. Seule une fenêtre
    /// visible et au niveau normal compte : celles du clavier ou du système ne
    /// présentent rien.
    #[allow(deprecated)]
    fn top_controller(mtm: MainThreadMarker) -> Option<Retained<UIViewController>> {
        let app = UIApplication::sharedApplication(mtm);
        let mut windows = Vec::new();
        let view = VIEW.load(Ordering::Acquire);
        if view != 0 {
            // SAFETY: l'adresse est celle de la UIView de la fenêtre d'eframe, qui vit
            // aussi longtemps que l'app ; on est sur le fil principal.
            let view = unsafe { &*(view as *const UIView) };
            windows.extend(view.window());
        }
        let mut others = Vec::new();
        for scene in app.connectedScenes().iter() {
            if let Ok(window_scene) = scene.downcast::<UIWindowScene>() {
                others.extend(window_scene.windows().iter());
            }
        }
        others.extend(app.keyWindow());
        others.extend(app.windows().iter());
        others.retain(|w| !w.isHidden() && w.windowLevel() == 0.0);
        // La fenêtre principale d'abord, puis n'importe laquelle qui a un contrôleur.
        others.sort_by_key(|w| !w.isKeyWindow());
        windows.extend(others);
        let mut top = windows.iter().find_map(|w| w.rootViewController())?;
        while let Some(next) = top.presentedViewController() {
            // Un contrôleur qui s'en va (un sélecteur qu'on ferme) ne présente plus rien.
            if next.isBeingDismissed() {
                break;
            }
            top = next;
        }
        Some(top)
    }

    unsafe extern "C" {
        fn dlopen(path: *const std::ffi::c_char, mode: std::ffi::c_int) -> *mut std::ffi::c_void;
    }

    /// Ajoute le fichier à la Pellicule (Photos.framework, chargé au besoin) :
    /// `PHAssetCreationRequest` depuis le fichier, dans un changement de la photothèque.
    pub fn save(path: &std::path::Path) {
        let library = AnyClass::get(c"PHPhotoLibrary").or_else(|| {
            // SAFETY: dlopen d'un framework du système, par son chemin fixe ; le
            // pointeur rendu n'est pas utilisé (le framework reste chargé).
            let _ = unsafe { dlopen(c"/System/Library/Frameworks/Photos.framework/Photos".as_ptr(), 1) };
            AnyClass::get(c"PHPhotoLibrary")
        });
        let (Some(library), Some(request)) = (library, AnyClass::get(c"PHAssetCreationRequest")) else {
            SAVE_FAILED.fetch_add(1, Ordering::AcqRel);
            return;
        };
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        // Le changement : une nouvelle photo, faite du fichier (copié par Photos).
        let changes = RcBlock::new(move || {
            // SAFETY: méthode de classe documentée de PHAssetCreationRequest, appelée
            // dans le bloc de `performChanges:` comme l'exige Photos ; `url` est une URL
            // de fichier retenue par le bloc.
            let _: *mut AnyObject = unsafe { msg_send![request, creationRequestForAssetFromImageAtFileURL: &*url] };
        });
        let done = RcBlock::new(|ok: objc2::runtime::Bool, _error: *mut NSError| {
            if ok.as_bool() {
                SAVED.fetch_add(1, Ordering::AcqRel);
            } else {
                SAVE_FAILED.fetch_add(1, Ordering::AcqRel);
            }
        });
        // SAFETY: `sharedPhotoLibrary` puis `performChanges:completionHandler:`, les
        // signatures de PHPhotoLibrary (un bloc sans argument, un bloc (BOOL, NSError*)) ;
        // Photos retient les blocs le temps du changement.
        unsafe {
            let shared: Retained<AnyObject> = msg_send![library, sharedPhotoLibrary];
            let _: () = msg_send![&*shared, performChanges: &*changes, completionHandler: &*done];
        }
    }

    /// La classe du sélecteur ; si PhotosUI n'est pas encore chargé, on le charge.
    fn picker_class() -> Option<&'static AnyClass> {
        AnyClass::get(c"PHPickerViewController").or_else(|| {
            // SAFETY: dlopen d'un framework du système, par son chemin fixe ; le
            // pointeur rendu n'est pas utilisé (le framework reste chargé).
            let _ = unsafe { dlopen(c"/System/Library/Frameworks/PhotosUI.framework/PhotosUI".as_ptr(), 1) };
            AnyClass::get(c"PHPickerViewController")
        })
    }

    pub fn pick(dest: PathBuf) -> Result<String, &'static str> {
        let mtm = MainThreadMarker::new().ok_or("hors du fil principal")?;
        let class = picker_class().ok_or("PhotosUI introuvable")?;
        let top = top_controller(mtm).ok_or("fenêtre introuvable")?;
        // SAFETY: configuration du sélecteur, objets neufs ; « Current » garde le fichier
        // d'origine (pas de conversion en JPEG), 0 = autant de photos qu'on veut.
        let config = unsafe {
            let config = PHPickerConfiguration::new();
            config.setSelectionLimit(0);
            config.setFilter(Some(&PHPickerFilter::imagesFilter()));
            config.setPreferredAssetRepresentationMode(PHPickerConfigurationAssetRepresentationMode::Current);
            config
        };
        // Le délégué se déclare conforme au protocole du sélecteur, que celui‑ci peut
        // vérifier avant de lui répondre.
        if let Some(protocol) = AnyProtocol::get(c"PHPickerViewControllerDelegate") {
            // SAFETY: ajouter un protocole à une classe déjà inscrite est permis ; sans
            // effet s'il y est déjà.
            unsafe {
                objc2::ffi::class_addProtocol(Delegate::class() as *const AnyClass as *mut AnyClass, protocol);
            }
        }
        let delegate = Delegate::alloc(mtm).set_ivars(Ivars { dest });
        // SAFETY: `init` de NSObject sur une instance neuve.
        let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
        // SAFETY: PHPickerViewController est un UIViewController de PhotosUI, créé par
        // son initialiseur documenté, avec un délégué qui répond à
        // `picker:didFinishPicking:` et qu'on retient (DELEGATE).
        let picker: Retained<AnyObject> = unsafe {
            let alloc: Allocated<AnyObject> = msg_send![class, alloc];
            let picker: Retained<AnyObject> = msg_send![alloc, initWithConfiguration: &*config];
            let _: () = msg_send![&picker, setDelegate: &*delegate];
            picker
        };
        DELEGATE.with(|d| *d.borrow_mut() = Some(delegate));
        let name = top.class().name().to_string_lossy().into_owned();
        // SAFETY: `picker` est un UIViewController neuf ; `top` un contrôleur de l'app,
        // sur le fil principal.
        present(&top, &picker);
        // iOS peut refuser sans rien dire ; il ne confirme qu'une fois le sélecteur à
        // l'écran (Photos tourne à part et prend parfois plus d'une seconde) : voir
        // `watch`.
        PENDING.with(|p| {
            *p.borrow_mut() = Some(Pending {
                picker,
                since: std::time::Instant::now(),
                rescued: false,
                told: false,
                top,
            })
        });
        Ok(name)
    }

    /// L'app Fichiers en mode import : le système copie chaque fichier choisi dans le
    /// dossier temporaire de LÜMA, puis `FilesDelegate` le range dans `dest`.
    #[allow(deprecated)] // L'initialiseur par types UTI, sans la crate des UTType.
    pub fn pick_files(dest: PathBuf) -> Result<(), &'static str> {
        let mtm = MainThreadMarker::new().ok_or("hors du fil principal")?;
        let top = top_controller(mtm).ok_or("fenêtre introuvable")?;
        let types = NSArray::from_retained_slice(&[
            NSString::from_str("public.camera-raw-image"),
            NSString::from_str("public.image"),
        ]);
        let picker = UIDocumentPickerViewController::initWithDocumentTypes_inMode(
            UIDocumentPickerViewController::alloc(mtm),
            &types,
            objc2_ui_kit::UIDocumentPickerMode::Import,
        );
        let delegate = FilesDelegate::alloc(mtm).set_ivars(Ivars { dest });
        // SAFETY: `init` de NSObject sur une instance neuve.
        let delegate: Retained<FilesDelegate> = unsafe { msg_send![super(delegate), init] };
        picker.setAllowsMultipleSelection(true);
        // SAFETY: le délégué est retenu (FILES_DELEGATE) tant que l'app Fichiers peut
        // lui répondre ; `top` est un contrôleur de l'app, sur le fil principal.
        unsafe {
            picker.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*delegate)));
            let _: () =
                msg_send![&top, presentViewController: &*picker, animated: true, completion: None::<&Block<dyn Fn()>>];
        }
        FILES_DELEGATE.with(|d| *d.borrow_mut() = Some(delegate));
        Ok(())
    }

    /// Vrai quand iOS a confirmé que le sélecteur est à l'écran (bloc de fin de la
    /// présentation).
    static SHOWN: AtomicBool = AtomicBool::new(false);

    /// Présente `picker` depuis `from` ; `SHOWN` passe à vrai quand il est à l'écran.
    fn present(from: &UIViewController, picker: &AnyObject) {
        SHOWN.store(false, Ordering::Release);
        let done = RcBlock::new(|| SHOWN.store(true, Ordering::Release));
        // SAFETY: `picker` est un UIViewController qui n'est présenté nulle part ;
        // `from` un contrôleur de l'app, sur le fil principal ; le bloc ne capture rien.
        unsafe {
            let _: () = msg_send![from, presentViewController: picker, animated: true, completion: &*done];
        }
    }

    /// Le sélecteur est‑il en route vers l'écran (présentation commencée) ?
    fn under_way(picker: &AnyObject) -> bool {
        // SAFETY: propriétés d'un UIViewController, sur le fil principal.
        unsafe {
            let presenting: Option<Retained<UIViewController>> = msg_send![picker, presentingViewController];
            let being: bool = msg_send![picker, isBeingPresented];
            presenting.is_some() || being
        }
    }

    pub fn pending() -> bool {
        PENDING.with(|p| p.borrow().is_some())
    }

    pub fn watch() -> Option<&'static str> {
        let mtm = MainThreadMarker::new()?;
        PENDING.with(|p| {
            let mut p = p.borrow_mut();
            let pending = p.as_mut()?;
            if SHOWN.load(Ordering::Acquire) {
                if pending.told {
                    return None;
                }
                pending.told = true;
                return Some(if pending.rescued {
                    "Photos à l'écran (fenêtre de secours)"
                } else {
                    "Photos à l'écran"
                });
            }
            // Pas encore confirmé : on laisse à iOS le temps de lancer Photos, et on
            // ne touche à rien tant que la présentation est en route.
            if pending.since.elapsed() < std::time::Duration::from_secs(5) || under_way(&pending.picker) {
                return None;
            }
            if pending.rescued {
                // Même la fenêtre de secours n'a rien montré : on abandonne.
                *p = None;
                PRESENTER.with(|w| {
                    if let Some(w) = w.borrow_mut().take() {
                        w.setHidden(true);
                    }
                });
                return Some("Photos ne s'affiche pas");
            }
            let fallback = presenter(mtm, &pending.top)?;
            present(&fallback, &pending.picker);
            pending.rescued = true;
            pending.since = std::time::Instant::now();
            Some("Photos refusé par la fenêtre de LÜMA : fenêtre de secours")
        })
    }

    /// Une fenêtre de secours au‑dessus de tout, dans la scène de LÜMA, avec un
    /// contrôleur vide qui présentera le sélecteur ; retenue jusqu'à sa fermeture.
    fn presenter(mtm: MainThreadMarker, near: &UIViewController) -> Option<Retained<UIViewController>> {
        let scene = near
            .viewIfLoaded()
            .and_then(|v| v.window())
            .and_then(|w| w.windowScene());
        let window: Retained<UIWindow> = match scene {
            // SAFETY: initialiseurs documentés de UIWindow, sur le fil principal.
            Some(scene) => unsafe { msg_send![UIWindow::alloc(mtm), initWithWindowScene: &*scene] },
            // SAFETY: idem, sans scène (l'écran entier par défaut).
            None => unsafe { msg_send![UIWindow::alloc(mtm), init] },
        };
        let controller = UIViewController::new(mtm);
        window.setWindowLevel(1.0);
        window.setRootViewController(Some(&controller));
        window.makeKeyAndVisible();
        PRESENTER.with(|w| *w.borrow_mut() = Some(window));
        Some(controller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_photo_never_overwrites_another() {
        let dir = std::env::temp_dir().join(format!("luma-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_name(&dir, "IMG_0001.DNG"), dir.join("IMG_0001.DNG"));
        std::fs::write(dir.join("IMG_0001.DNG"), b"1").unwrap();
        assert_eq!(free_name(&dir, "IMG_0001.DNG"), dir.join("IMG_0001 2.DNG"));
        std::fs::write(dir.join("IMG_0001 2.DNG"), b"2").unwrap();
        assert_eq!(free_name(&dir, "IMG_0001.DNG"), dir.join("IMG_0001 3.DNG"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
