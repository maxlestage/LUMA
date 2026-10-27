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
/// le fil principal. En cas d'échec, ce qui a manqué (pour le dire à l'utilisateur).
pub fn pick(dest: PathBuf) -> Result<(), &'static str> {
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

/// Le sélecteur de Photos existe‑t‑il (iPhone et iPad) ?
pub const AVAILABLE: bool = cfg!(target_os = "ios");

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
    use objc2_ui_kit::{UIApplication, UIViewController, UIWindowScene};

    use super::{CHOSEN, FAILED, IMPORTED, free_name};

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
                CHOSEN.fetch_add(results.count(), Ordering::Release);
                for result in results {
                    load(&result, self.ivars().dest.clone());
                }
            }
        }
    );

    thread_local! {
        /// Le délégué du sélecteur en cours : le sélecteur ne le retient pas.
        static DELEGATE: RefCell<Option<Retained<Delegate>>> = const { RefCell::new(None) };
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
    /// principale, cherchée dans les scènes, sinon parmi les fenêtres de l'app (une app
    /// sans scène déclarée n'en a pas toujours).
    #[allow(deprecated)]
    fn top_controller(mtm: MainThreadMarker) -> Option<Retained<UIViewController>> {
        let app = UIApplication::sharedApplication(mtm);
        let mut windows = Vec::new();
        for scene in app.connectedScenes().iter() {
            if let Ok(window_scene) = scene.downcast::<UIWindowScene>() {
                windows.extend(window_scene.windows().iter());
            }
        }
        windows.extend(app.keyWindow());
        windows.extend(app.windows().iter());
        // La fenêtre principale d'abord, puis n'importe laquelle qui a un contrôleur.
        windows.sort_by_key(|w| !w.isKeyWindow());
        let mut top = windows.iter().find_map(|w| w.rootViewController())?;
        while let Some(next) = top.presentedViewController() {
            top = next;
        }
        Some(top)
    }

    unsafe extern "C" {
        fn dlopen(path: *const std::ffi::c_char, mode: std::ffi::c_int) -> *mut std::ffi::c_void;
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

    pub fn pick(dest: PathBuf) -> Result<(), &'static str> {
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
        unsafe {
            let alloc: Allocated<AnyObject> = msg_send![class, alloc];
            let picker: Retained<AnyObject> = msg_send![alloc, initWithConfiguration: &*config];
            let _: () = msg_send![&picker, setDelegate: &*delegate];
            let _: () =
                msg_send![&top, presentViewController: &*picker, animated: true, completion: None::<&Block<dyn Fn()>>];
        }
        DELEGATE.with(|d| *d.borrow_mut() = Some(delegate));
        Ok(())
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
