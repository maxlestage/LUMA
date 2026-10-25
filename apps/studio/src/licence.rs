//! LÜMA Premium dans l'app : l'état enregistré (`licence.json`, à côté des films et des
//! préréglages), la clé signée vérifiée hors ligne, la clé fraîche demandée chaque jour
//! en arrière‑plan, et l'écran Premium (les formules, la clé).
//!
//! Le réseau passe par `curl` (présent sur macOS et Linux) : pas de bibliothèque TLS
//! compilée dans LÜMA. Sur iPhone et iPad, la clé est vérifiée hors ligne et vaut
//! jusqu'à son terme ; l'abonnement y passera par l'App Store (achats intégrés).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use luma_licence::{Access, Plan, Refused, State, Transport};

use crate::i18n::{tr, trf};

/// Maintenant, en secondes Unix.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Le dossier de `licence.json` : celui de LÜMA (au‑dessus de Films), ou
/// `LUMA_LICENCE_DIR`.
fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("LUMA_LICENCE_DIR") {
        return Some(PathBuf::from(d));
    }
    crate::dialogs::app_dir()
}

/// Les requêtes à l'API de licences, par `curl`.
pub struct Curl;

impl Transport for Curl {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
        if cfg!(target_os = "ios") {
            return Err(tr("pas de vérification en ligne sur iPhone et iPad").into());
        }
        let out = std::process::Command::new("curl")
            .args(["-sS", "--fail-with-body", "--max-time", "20", "-X", "POST"])
            .args([
                "-H",
                "Accept: application/json",
                "-A",
                concat!("LUMA/", env!("CARGO_PKG_VERSION")),
            ])
            .args(["--data", &luma_licence::form_encode(form), url])
            .output()
            .map_err(|e| format!("curl : {e}"))?;
        let body = String::from_utf8_lossy(&out.stdout).into_owned();
        // L'API répond en JSON même pour une clé refusée (code 4xx) : on le garde.
        if body.trim_start().starts_with('{') {
            Ok(body)
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }
}

/// Ouvre une page (paiement, compte client) dans le navigateur. Sur iPhone et iPad, le
/// lien est copié : `false`.
pub fn open_url(url: &str) -> bool {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "ios") {
        return false;
    } else {
        "xdg-open"
    };
    std::process::Command::new(program).arg(url).spawn().is_ok()
}

/// Le résultat d'une opération en ligne : le nouvel état, et un message (ou le refus :
/// sans réseau ?, et ce qu'il faut en dire).
type Done = (State, Result<String, (bool, String)>);

/// Ce que l'app dit d'un refus, dans la langue de l'interface. Les messages de
/// `luma-licence` (et ceux du serveur) sont en français : ce sont des clés du catalogue.
pub fn refusal(e: &Refused) -> String {
    match e {
        Refused::Offline(detail) => trf(
            "impossible de joindre le serveur de licences ({detail})",
            &[("detail", &tr(detail))],
        ),
        Refused::Invalid(message) => tr(message).to_owned(),
    }
}

/// LÜMA Premium : l'état, et l'opération en ligne en cours.
pub struct Licence {
    pub state: State,
    path: Option<PathBuf>,
    pending: Option<Receiver<Done>>,
    /// Le champ de la clé.
    pub key_input: String,
    /// Le dernier message (clé activée, refusée…).
    pub message: String,
}

impl Licence {
    /// Lit `licence.json` (ou commence l'essai), et lance la vérification du jour.
    pub fn load() -> Licence {
        Self::load_from(dir())
    }

    pub fn load_from(dir: Option<PathBuf>) -> Licence {
        let path = dir.map(|d| {
            let _ = std::fs::create_dir_all(&d);
            d.join("licence.json")
        });
        let state = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| State::from_json(&t));
        let fresh = state.is_none();
        let mut l = Licence {
            state: state.unwrap_or_else(|| State::new(now())),
            path,
            pending: None,
            key_input: String::new(),
            message: String::new(),
        };
        if fresh {
            l.save();
        }
        l.refresh_if_due();
        l
    }

    /// Une fois par jour, en ligne : une clé fraîche tant que l'abonnement est actif.
    fn refresh_if_due(&mut self) {
        if let Some(server) = luma_licence::licence_server()
            && self.state.needs_check(now())
            && !cfg!(target_os = "ios")
        {
            self.run(move |s| {
                luma_licence::refresh(s, server, luma_licence::public_key().as_ref(), &Curl, now())
                    .map(|()| String::new())
            });
        }
    }

    pub fn access(&self) -> Access {
        self.state
            .access(now(), luma_licence::public_key().as_ref())
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    fn save(&self) {
        if let Some(p) = &self.path {
            let _ = std::fs::write(p, self.state.to_json());
        }
    }

    /// Lance `op` sur une copie de l'état, dans un fil à part.
    fn run(&mut self, op: impl FnOnce(&mut State) -> Result<String, Refused> + Send + 'static) {
        let (tx, rx) = channel();
        let mut state = self.state.clone();
        std::thread::spawn(move || {
            let r = op(&mut state).map_err(|e| (matches!(e, Refused::Offline(_)), refusal(&e)));
            let _ = tx.send((state, r));
        });
        self.pending = Some(rx);
    }

    /// À chaque image : récupère le résultat d'une opération finie. `true` si l'état a
    /// changé.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.pending else {
            return false;
        };
        let Ok((state, result)) = rx.try_recv() else {
            return false;
        };
        self.pending = None;
        // Sans réseau, la vérification du jour ne change rien et ne dit rien.
        let offline = matches!(&result, Err((true, _)));
        if !offline || state != self.state {
            self.state = state;
            self.save();
        }
        self.message = match result {
            Ok(m) => m,
            Err(_) if offline && self.state.key.is_some() && self.access().is_allowed() => {
                String::new()
            }
            Err((_, e)) => e,
        };
        true
    }

    /// Active la clé saisie (vérifiée tout de suite, hors ligne).
    pub fn activate(&mut self) {
        let key = self.key_input.trim().to_owned();
        match luma_licence::activate(
            &mut self.state,
            &key,
            luma_licence::public_key().as_ref(),
            now(),
        ) {
            Ok(c) => {
                self.save();
                self.key_input.clear();
                self.message = if c.email.is_empty() {
                    tr("Bienvenue dans LÜMA Premium.").to_owned()
                } else {
                    trf(
                        "Bienvenue dans LÜMA Premium, {email}.",
                        &[("email", &c.email)],
                    )
                };
            }
            Err(e) => self.message = refusal(&e),
        }
    }

    /// Retire la clé de cet appareil (elle reste valable ailleurs).
    pub fn deactivate(&mut self) {
        luma_licence::forget(&mut self.state);
        self.save();
        self.message = tr("Clé retirée de cet appareil.").into();
    }

    /// Ce que dit la puce de la barre du haut.
    pub fn badge(&self) -> String {
        match self.access() {
            Access::Premium { .. } => "Premium".into(),
            Access::Trial { days_left } => trf("Essai · {n} j", &[("n", &days_left)]),
            Access::Expired { .. } => tr("Essai terminé").into(),
        }
    }
}

/// Ce que LÜMA Premium comprend (traduit à l'affichage).
pub const FEATURES: [&str; 6] = [
    "Le développement RAW complet, sur le GPU",
    "100 films, vos préréglages, et vos pellicules mesurées",
    "Masques, films empilés, recadrage et géométrie",
    "Empilements, ciel étoilé, panoramas en DNG",
    "Export JPEG, PNG, TIFF 16 bits, en P3 et ProPhoto",
    "Mac, iPhone et iPad, et toutes les mises à jour",
];

/// Les formules, dans l'ordre d'affichage.
pub const PLANS: [Plan; 2] = Plan::ALL;
