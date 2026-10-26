//! La prise de vue en connexion sur Mac : les boîtiers reliés (USB, ou réseau pour
//! ceux qui le permettent), par ImageCaptureCore, le cadre du système (celui de
//! Transfert d'images). Chaque photo prise boîtier relié est copiée telle quelle dans
//! le dossier donné, où le dossier surveillé de LÜMA la reprend.
//!
//! Les photos déjà sur la carte ne sont pas copiées : seules comptent celles qui
//! arrivent une fois le contenu de la carte connu. [`trigger`] déclenche les boîtiers
//! qui le permettent.
//!
//! Ailleurs que sur Mac, il n'y a pas de boîtier relié : [`start`] rend `false`.

use std::path::PathBuf;
use std::sync::Mutex;

/// La prise de vue en connexion existe‑t‑elle (Mac) ?
pub const AVAILABLE: bool = cfg!(target_os = "macos");

/// Où en est la connexion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// La connexion est demandée (entre [`start`] et [`stop`]).
    pub running: bool,
    /// Les boîtiers reliés, par nom.
    pub cameras: Vec<String>,
    /// Les photos copiées depuis [`start`].
    pub received: usize,
    /// Un boîtier au moins peut être déclenché depuis LÜMA.
    pub can_trigger: bool,
}

static STATUS: Mutex<Status> = Mutex::new(Status {
    running: false,
    cameras: Vec::new(),
    received: 0,
    can_trigger: false,
});

fn update(f: impl FnOnce(&mut Status)) {
    if let Ok(mut s) = STATUS.lock() {
        f(&mut s);
    }
}

pub fn status() -> Status {
    STATUS.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Cherche les boîtiers reliés ; leurs nouvelles photos arriveront dans `dest`. À
/// appeler sur le fil principal. `false` ailleurs que sur Mac.
pub fn start(dest: PathBuf) -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::start(dest)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = dest;
        false
    }
}

/// Ferme les boîtiers et arrête la recherche. Sur le fil principal.
pub fn stop() {
    #[cfg(target_os = "macos")]
    mac::stop();
    update(|s| *s = Status::default());
}

/// Déclenche les boîtiers reliés qui le permettent. Sur le fil principal.
pub fn trigger() {
    #[cfg(target_os = "macos")]
    mac::trigger();
}

#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
    use objc2_core_graphics::CGImage;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString, NSURL};
    use objc2_image_capture_core::{
        ICCameraDevice, ICCameraDeviceCanTakePicture, ICCameraDeviceDelegate, ICCameraFile, ICCameraItem, ICDevice,
        ICDeviceBrowser, ICDeviceBrowserDelegate, ICDeviceDelegate, ICDeviceLocationTypeMask, ICDeviceTypeMask,
        ICDownloadsDirectoryURL,
    };

    use super::update;

    struct Ivars {
        dest: PathBuf,
        /// Les boîtiers ouverts ; `true` une fois le contenu de leur carte connu.
        cameras: RefCell<Vec<(Retained<ICCameraDevice>, bool)>>,
    }

    define_class!(
        // SAFETY: NSObject n'impose rien à ses sous‑classes ; `Delegate` n'implémente pas
        // `Drop` et ne vit que sur le fil principal, où ImageCaptureCore appelle ses
        // délégués.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LumaTetherDelegate"]
        #[ivars = Ivars]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        // SAFETY: les signatures sont celles de `ICDeviceBrowserDelegate`.
        unsafe impl ICDeviceBrowserDelegate for Delegate {
            #[unsafe(method(deviceBrowser:didAddDevice:moreComing:))]
            fn did_add_device(&self, _browser: &ICDeviceBrowser, device: &ICDevice, _more: bool) {
                let Some(camera) = device.downcast_ref::<ICCameraDevice>() else {
                    return;
                };
                // SAFETY: le boîtier vient du système ; le délégué vit tant que la
                // recherche (BROWSER le retient), et le boîtier ne le retient pas.
                unsafe {
                    camera.setDelegate(Some(ProtocolObject::from_ref(self)));
                    camera.requestOpenSession();
                }
                self.ivars().cameras.borrow_mut().push((camera.retain(), false));
                self.refresh();
            }

            #[unsafe(method(deviceBrowser:didRemoveDevice:moreGoing:))]
            fn did_remove_device(&self, _browser: &ICDeviceBrowser, device: &ICDevice, _more: bool) {
                self.forget(device);
            }
        }

        // SAFETY: les signatures sont celles de `ICDeviceDelegate`.
        unsafe impl ICDeviceDelegate for Delegate {
            #[unsafe(method(device:didCloseSessionWithError:))]
            fn did_close(&self, device: &ICDevice, _error: Option<&NSError>) {
                self.forget(device);
            }

            #[unsafe(method(didRemoveDevice:))]
            fn did_remove(&self, device: &ICDevice) {
                self.forget(device);
            }

            #[unsafe(method(device:didOpenSessionWithError:))]
            fn did_open(&self, device: &ICDevice, error: Option<&NSError>) {
                if error.is_some() {
                    self.forget(device);
                }
            }
        }

        // SAFETY: les signatures sont celles de `ICCameraDeviceDelegate` ; les méthodes
        // obligatoires sans usage ici ne font rien.
        unsafe impl ICCameraDeviceDelegate for Delegate {
            #[unsafe(method(cameraDevice:didAddItems:))]
            fn did_add_items(&self, camera: &ICCameraDevice, items: &NSArray<ICCameraItem>) {
                // Les photos déjà sur la carte arrivent avant que son contenu soit connu :
                // elles ne sont pas copiées.
                let ready = self
                    .ivars()
                    .cameras
                    .borrow()
                    .iter()
                    .any(|(c, ready)| *ready && std::ptr::eq(&**c, camera));
                if !ready {
                    return;
                }
                for item in items.iter() {
                    if let Some(file) = item.downcast_ref::<ICCameraFile>() {
                        download(file, &self.ivars().dest);
                    }
                }
            }

            #[unsafe(method(cameraDevice:didRemoveItems:))]
            fn did_remove_items(&self, _camera: &ICCameraDevice, _items: &NSArray<ICCameraItem>) {}

            #[unsafe(method(cameraDevice:didReceiveThumbnail:forItem:error:))]
            fn did_receive_thumbnail(
                &self,
                _camera: &ICCameraDevice,
                _thumbnail: Option<&CGImage>,
                _item: &ICCameraItem,
                _error: Option<&NSError>,
            ) {
            }

            #[unsafe(method(cameraDevice:didReceiveMetadata:forItem:error:))]
            fn did_receive_metadata(
                &self,
                _camera: &ICCameraDevice,
                _metadata: Option<&NSDictionary>,
                _item: &ICCameraItem,
                _error: Option<&NSError>,
            ) {
            }

            #[unsafe(method(cameraDevice:didRenameItems:))]
            fn did_rename_items(&self, _camera: &ICCameraDevice, _items: &NSArray<ICCameraItem>) {}

            #[unsafe(method(cameraDeviceDidChangeCapability:))]
            fn did_change_capability(&self, _camera: &ICCameraDevice) {
                self.refresh();
            }

            #[unsafe(method(cameraDevice:didReceivePTPEvent:))]
            fn did_receive_ptp_event(&self, _camera: &ICCameraDevice, _event: &NSData) {}

            #[unsafe(method(deviceDidBecomeReadyWithCompleteContentCatalog:))]
            fn did_become_ready(&self, camera: &ICCameraDevice) {
                for (c, ready) in self.ivars().cameras.borrow_mut().iter_mut() {
                    if std::ptr::eq(&**c, camera) {
                        *ready = true;
                    }
                }
                // La prise de vue en connexion est active d'elle‑même sur les boîtiers
                // qui se déclenchent à distance.
                self.refresh();
            }

            #[unsafe(method(cameraDeviceDidRemoveAccessRestriction:))]
            fn did_remove_restriction(&self, _device: &ICDevice) {}

            #[unsafe(method(cameraDeviceDidEnableAccessRestriction:))]
            fn did_enable_restriction(&self, _device: &ICDevice) {}
        }
    );

    impl Delegate {
        fn new(mtm: MainThreadMarker, dest: PathBuf) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars {
                dest,
                cameras: RefCell::new(Vec::new()),
            });
            // SAFETY: `init` de NSObject, sur un objet qui vient d'être alloué.
            unsafe { msg_send![super(this), init] }
        }

        fn forget(&self, device: &ICDevice) {
            self.ivars()
                .cameras
                .borrow_mut()
                .retain(|(c, _)| !std::ptr::eq::<ICDevice>(&***c, device));
            self.refresh();
        }

        /// Le nom des boîtiers et ce qu'ils permettent, pour l'app.
        fn refresh(&self) {
            let cameras = self.ivars().cameras.borrow();
            // SAFETY: lectures sur des boîtiers que le système a donnés et qu'on retient.
            let names: Vec<String> = cameras
                .iter()
                .map(|(c, _)| unsafe { c.name() }.map_or_else(String::new, |n| n.to_string()))
                .collect();
            // SAFETY: une constante du cadre, lue après son chargement.
            let take_picture = unsafe { ICCameraDeviceCanTakePicture };
            let can_trigger = cameras.iter().any(|(c, ready)| {
                // SAFETY: lecture sur un boîtier que le système a donné et qu'on retient.
                *ready && unsafe { c.capabilities() }.containsObject(take_picture)
            });
            update(|s| {
                s.cameras = names;
                s.can_trigger = can_trigger;
            });
        }
    }

    /// Copie une photo du boîtier dans `dest`, sous son nom.
    fn download(file: &ICCameraFile, dest: &std::path::Path) {
        if std::fs::create_dir_all(dest).is_err() {
            return;
        }
        let dir = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&dest.to_string_lossy()), true);
        let value: &AnyObject = &dir;
        // SAFETY: une constante du cadre, lue après son chargement.
        let key = unsafe { ICDownloadsDirectoryURL };
        let options = NSDictionary::from_slices(&[key], &[value]);
        let done = block2::RcBlock::new(|_name: *mut NSString, error: *mut NSError| {
            if error.is_null() {
                update(|s| s.received += 1);
            }
        });
        // SAFETY: les options sont un dictionnaire de clés de téléchargement vers des
        // objets ; le bloc ne capture rien et peut être appelé sur n'importe quel fil.
        let _ = unsafe { file.requestDownloadWithOptions_completion(Some(&options), &done) };
    }

    thread_local! {
        /// La recherche en cours et son délégué : le navigateur ne retient pas son
        /// délégué.
        static BROWSER: RefCell<Option<(Retained<ICDeviceBrowser>, Retained<Delegate>)>> = const { RefCell::new(None) };
    }

    pub fn start(dest: PathBuf) -> bool {
        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };
        stop();
        let delegate = Delegate::new(mtm, dest);
        // SAFETY: un navigateur neuf, sur le fil principal ; son délégué est retenu par
        // BROWSER aussi longtemps que lui.
        let browser = unsafe {
            let browser = ICDeviceBrowser::new();
            browser.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            // Les boîtiers reliés à ce Mac, et ceux du réseau qui le permettent.
            let location = ICDeviceLocationTypeMask::Local.0 | ICDeviceLocationTypeMask::Remote.0;
            browser.setBrowsedDeviceTypeMask(ICDeviceTypeMask(ICDeviceTypeMask::Camera.0 | location));
            browser.start();
            browser
        };
        BROWSER.with(|b| *b.borrow_mut() = Some((browser, delegate)));
        super::update(|s| {
            *s = super::Status {
                running: true,
                ..Default::default()
            }
        });
        true
    }

    pub fn stop() {
        let Some((browser, delegate)) = BROWSER.with(|b| b.borrow_mut().take()) else {
            return;
        };
        // SAFETY: le navigateur et les boîtiers ouverts par lui, sur le fil principal.
        unsafe {
            for (camera, _) in delegate.ivars().cameras.borrow().iter() {
                camera.requestCloseSession();
                camera.setDelegate(None);
            }
            browser.stop();
            browser.setDelegate(None);
        }
    }

    pub fn trigger() {
        BROWSER.with(|b| {
            if let Some((_, delegate)) = &*b.borrow() {
                for (camera, ready) in delegate.ivars().cameras.borrow().iter() {
                    // SAFETY: lecture sur un boîtier ouvert, retenu par le délégué.
                    if *ready && unsafe { camera.tetheredCaptureEnabled() } {
                        // SAFETY: un boîtier ouvert dont le contenu est connu ; la demande
                        // est sans effet s'il ne se déclenche pas à distance.
                        unsafe { camera.requestTakePicture() };
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_to_connect_off_mac() {
        if AVAILABLE {
            return;
        }
        assert!(!start(std::env::temp_dir()));
        trigger();
        stop();
        assert_eq!(status(), Status::default());
    }
}
