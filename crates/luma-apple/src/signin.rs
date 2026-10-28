//! « Se connecter avec Apple » sur iPhone et iPad : l'accès offert.
//!
//! LÜMA Premium ne s'achète sur iPhone et iPad que dans l'App Store ; les personnes à
//! qui il est offert (la liste `LUMA_OFFERTS` du serveur) l'ouvrent en se connectant
//! avec leur compte Apple. [`start`] affiche la feuille du système (Face ID, le
//! courriel partagé ou masqué) ; ce qui revient arrive par le canal rendu, en
//! [`Event`] : le jeton d'identité signé par Apple, que l'app envoie au serveur, qui le
//! vérifie et rend une clé offerte si le courriel est dans la liste.
//!
//! Comme StoreKit dans `store`, l'interface Objective‑C d'AuthenticationServices est
//! appelée depuis Rust. Ailleurs qu'iPhone et iPad, [`start`] rend `None`.

use std::sync::mpsc::Receiver;

/// Ce que répond la feuille « Se connecter avec Apple ».
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Connecté : le jeton d'identité (un JWT signé par Apple, valable quelques
    /// minutes), qui porte le courriel.
    Token(String),
    /// La personne a fermé la feuille.
    Cancelled,
    /// Le système a refusé : son message.
    Failed(String),
}

/// « Se connecter avec Apple » est‑il là (iPhone et iPad) ?
pub const AVAILABLE: bool = cfg!(target_os = "ios");

/// Affiche la feuille « Se connecter avec Apple », en demandant le courriel. À appeler
/// sur le fil principal ; `None` ailleurs qu'iPhone et iPad.
pub fn start() -> Option<Receiver<Event>> {
    #[cfg(target_os = "ios")]
    {
        ios::start()
    }
    #[cfg(not(target_os = "ios"))]
    {
        None
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use std::cell::RefCell;
    use std::sync::mpsc::{Receiver, Sender, channel};

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_authentication_services::{
        ASAuthorization, ASAuthorizationAppleIDCredential, ASAuthorizationAppleIDProvider, ASAuthorizationController,
        ASAuthorizationControllerDelegate, ASAuthorizationError, ASAuthorizationRequest, ASAuthorizationScopeEmail,
    };
    use objc2_foundation::{NSArray, NSError};
    use objc2_ui_kit::{UIApplication, UIWindow, UIWindowScene};

    use super::Event;

    struct Ivars {
        tx: Sender<Event>,
    }

    define_class!(
        // SAFETY: NSObject n'impose rien à ses sous‑classes, et `Delegate` n'implémente
        // pas `Drop`. AuthenticationServices l'appelle sur le fil principal.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LumaAppleSignIn"]
        #[ivars = Ivars]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        // SAFETY: les signatures sont celles du protocole (objc2-authentication-services).
        unsafe impl ASAuthorizationControllerDelegate for Delegate {
            #[unsafe(method(authorizationController:didCompleteWithAuthorization:))]
            fn completed(&self, _controller: &ASAuthorizationController, authorization: &ASAuthorization) {
                // SAFETY: lecture du justificatif que le système vient de rendre ; pour
                // « Se connecter avec Apple », c'est un ASAuthorizationAppleIDCredential.
                let token = unsafe {
                    let credential = authorization.credential();
                    let object: &objc2::runtime::AnyObject = credential.as_ref();
                    object
                        .downcast_ref::<ASAuthorizationAppleIDCredential>()
                        .and_then(|c| c.identityToken())
                        .and_then(|data| String::from_utf8(data.to_vec()).ok())
                };
                let event = match token {
                    Some(t) => Event::Token(t),
                    None => Event::Failed("Apple n'a pas rendu de jeton d'identité".into()),
                };
                self.finish(event);
            }

            #[unsafe(method(authorizationController:didCompleteWithError:))]
            fn failed(&self, _controller: &ASAuthorizationController, error: &NSError) {
                let event = if error.code() == ASAuthorizationError::Canceled.0 {
                    Event::Cancelled
                } else {
                    Event::Failed(error.localizedDescription().to_string())
                };
                self.finish(event);
            }
        }

        // La fenêtre où la feuille s'affiche : objc2 ne déclare cette méthode du
        // protocole que pour le Mac (ASPresentationAnchor y est une NSWindow ; sur
        // iPhone et iPad, une UIWindow).
        impl Delegate {
            #[unsafe(method_id(presentationAnchorForAuthorizationController:))]
            fn anchor(&self, _controller: &ASAuthorizationController) -> Retained<UIWindow> {
                window(self.mtm())
            }
        }
    );

    impl Delegate {
        fn new(mtm: MainThreadMarker, tx: Sender<Event>) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars { tx });
            // SAFETY: `init` de NSObject, sur un objet tout juste alloué.
            unsafe { msg_send![super(this), init] }
        }

        fn finish(&self, event: Event) {
            let _ = self.ivars().tx.send(event);
            KEEP.with(|k| k.borrow_mut().take());
        }
    }

    /// La fenêtre principale de l'app (la première visible au niveau normal).
    #[allow(deprecated)]
    fn window(mtm: MainThreadMarker) -> Retained<UIWindow> {
        let app = UIApplication::sharedApplication(mtm);
        let mut windows: Vec<Retained<UIWindow>> = Vec::new();
        windows.extend(app.keyWindow());
        for scene in app.connectedScenes().iter() {
            if let Ok(scene) = scene.downcast::<UIWindowScene>() {
                windows.extend(scene.windows().iter());
            }
        }
        windows
            .into_iter()
            .find(|w| !w.isHidden() && w.windowLevel() == 0.0)
            .unwrap_or_else(|| UIWindow::new(mtm))
    }

    thread_local! {
        // Le contrôleur et son délégué vivent jusqu'à la réponse : le système ne
        // retient pas le délégué.
        static KEEP: RefCell<Option<(Retained<ASAuthorizationController>, Retained<Delegate>)>> =
            const { RefCell::new(None) };
    }

    pub fn start() -> Option<Receiver<Event>> {
        let mtm = MainThreadMarker::new()?;
        let (tx, rx) = channel();
        let delegate = Delegate::new(mtm, tx);
        // SAFETY: une demande « Se connecter avec Apple » neuve (le courriel seulement),
        // remise à un contrôleur neuf dont le délégué est gardé en vie (KEEP) ; tout se
        // passe sur le fil principal.
        let controller = unsafe {
            let request = ASAuthorizationAppleIDProvider::new().createRequest();
            request.setRequestedScopes(Some(&NSArray::from_slice(&[ASAuthorizationScopeEmail])));
            let request: Retained<ASAuthorizationRequest> = Retained::into_super(Retained::into_super(request));
            let controller = ASAuthorizationController::initWithAuthorizationRequests(
                ASAuthorizationController::alloc(),
                &NSArray::from_retained_slice(&[request]),
            );
            controller.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            // Le fournisseur de fenêtre : le même objet, qui répond à
            // `presentationAnchorForAuthorizationController:`.
            let _: () = msg_send![&*controller, setPresentationContextProvider: &*delegate];
            controller.performRequests();
            controller
        };
        KEEP.with(|k| *k.borrow_mut() = Some((controller, delegate)));
        Some(rx)
    }
}
