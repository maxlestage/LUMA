//! LÜMA Premium dans l'app : l'état enregistré (`licence.json`, à côté des films et des
//! préréglages), la vérification en ligne en arrière‑plan, et l'écran Premium (les
//! formules, la clé).
//!
//! Le réseau passe par `curl` (présent sur macOS et Linux) : pas de bibliothèque TLS
//! compilée dans LÜMA. Sur iPhone et iPad, l'abonnement passera par l'App Store
//! (achats intégrés) ; en attendant, la clé y est acceptée sans vérification en ligne.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use luma_licence::{Access, Plan, Refused, State, Transport};

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
    crate::dialogs::films_dir()?.parent().map(PathBuf::from)
}

/// Les requêtes à l'API de licences, par `curl`.
pub struct Curl;

impl Transport for Curl {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
        if cfg!(target_os = "ios") {
            return Err("pas de vérification en ligne sur iPhone et iPad".into());
        }
        let out = std::process::Command::new("curl")
            .args(["-sS", "--fail-with-body", "--max-time", "20", "-X", "POST"])
            .args(["-H", "Accept: application/json"])
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

/// Le nom de cette machine, visible dans le compte client (pour savoir où la clé est
/// activée).
fn machine_name() -> String {
    let try_cmd = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    if cfg!(target_os = "macos")
        && let Some(n) = try_cmd("scutil", &["--get", "ComputerName"])
    {
        return n;
    }
    try_cmd("hostname", &[]).unwrap_or_else(|| "LÜMA".into())
}

/// Le résultat d'une opération en ligne : le nouvel état, et un message.
type Done = (State, Result<String, String>);

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
        if l.state.needs_check(now()) {
            l.run(|s| luma_licence::validate(s, &Curl, now()).map(|()| String::new()));
        }
        l
    }

    pub fn access(&self) -> Access {
        self.state.access(now())
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
            let r = op(&mut state).map_err(|e| e.to_string());
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
        let offline = matches!(&result, Err(e) if e.starts_with("impossible de joindre"));
        if !offline || state != self.state {
            self.state = state;
            self.save();
        }
        self.message = match result {
            Ok(m) => m,
            Err(e) if offline && self.state.key.is_some() && self.access().is_allowed() => {
                let _ = e;
                String::new()
            }
            Err(e) => e,
        };
        true
    }

    /// Active la clé saisie.
    pub fn activate(&mut self) {
        let key = self.key_input.trim().to_owned();
        if cfg!(target_os = "ios") {
            // En attendant l'App Store : la clé est gardée telle quelle.
            self.state.key = Some(key);
            self.state.instance_id = Some("ios".into());
            self.state.checked_at = now();
            self.save();
            self.message = "Clé enregistrée.".into();
            return;
        }
        let machine = machine_name();
        self.message = "Activation…".into();
        self.run(move |s| {
            luma_licence::activate(s, &key, &machine, &Curl, now())
                .map(|()| "Bienvenue dans LÜMA Premium.".to_owned())
        });
    }

    /// Retire la clé de cette machine.
    pub fn deactivate(&mut self) {
        self.run(|s| {
            luma_licence::deactivate(s, &Curl).map(|()| "Clé retirée de cette machine.".to_owned())
        });
    }

    /// Ce que dit la puce de la barre du haut.
    pub fn badge(&self) -> String {
        match self.access() {
            Access::Premium { .. } => "Premium".into(),
            Access::Trial { days_left } => format!("Essai · {days_left} j"),
            Access::Expired { .. } => "Essai terminé".into(),
        }
    }
}

/// Ce que LÜMA Premium comprend.
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
