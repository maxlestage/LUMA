//! LÜMA Premium : 14 jours d'essai, puis un abonnement, au mois ou à l'an.
//!
//! Le paiement, la TVA, les renouvellements et les résiliations sont confiés à Lemon
//! Squeezy (vendeur officiel, « merchant of record ») : à l'achat, il envoie une clé de
//! licence par courriel. L'app l'active une fois (`/v1/licenses/activate`), puis la
//! revérifie chaque jour en ligne (`/v1/licenses/validate`) ; hors ligne, elle reste
//! valable [`OFFLINE_GRACE_DAYS`] jours après la dernière vérification réussie. Une
//! clé d'abonnement résilié ou impayé est désactivée par Lemon Squeezy : l'app le voit
//! à la vérification suivante.
//!
//! Ce module ne fait pas de réseau : il décide (essai, abonné, expiré), lit les réponses
//! de l'API et dit quoi demander. L'app fournit le transport ([`Transport`]).
//!
//! La boutique se configure à la compilation, sans toucher au code :
//! `LUMA_CHECKOUT_MONTHLY`, `LUMA_CHECKOUT_ANNUAL` (liens de paiement) et
//! `LUMA_STORE_ID` (seules les clés de cette boutique sont acceptées).

use serde::{Deserialize, Serialize};

/// Durée de l'essai gratuit.
pub const TRIAL_DAYS: u64 = 14;
/// Combien de temps une clé reste valable sans pouvoir la revérifier (hors ligne).
pub const OFFLINE_GRACE_DAYS: u64 = 14;
/// Revérification en ligne au plus une fois par…
pub const RECHECK_HOURS: u64 = 24;

const DAY: u64 = 86_400;
const API: &str = "https://api.lemonsqueezy.com/v1/licenses";

/// Les deux formules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plan {
    Monthly,
    Annual,
}

impl Plan {
    pub const ALL: [Plan; 2] = [Plan::Monthly, Plan::Annual];

    pub fn name(self) -> &'static str {
        match self {
            Plan::Monthly => "Mensuel",
            Plan::Annual => "Annuel",
        }
    }

    /// Le prix affiché (TTC, en euros).
    pub fn price(self) -> &'static str {
        match self {
            Plan::Monthly => "9,99 €",
            Plan::Annual => "79,99 €",
        }
    }

    pub fn period(self) -> &'static str {
        match self {
            Plan::Monthly => "par mois",
            Plan::Annual => "par an",
        }
    }

    /// Ce que la formule a de plus.
    pub fn note(self) -> &'static str {
        match self {
            Plan::Monthly => "Sans engagement, résiliable à tout moment.",
            Plan::Annual => "Deux mois offerts : 6,67 € par mois.",
        }
    }

    /// Le lien de paiement, s'il est configuré pour cette version.
    pub fn checkout_url(self) -> Option<&'static str> {
        let url = match self {
            Plan::Monthly => option_env!("LUMA_CHECKOUT_MONTHLY"),
            Plan::Annual => option_env!("LUMA_CHECKOUT_ANNUAL"),
        };
        url.filter(|u| u.starts_with("https://"))
    }
}

/// La boutique dont les clés sont acceptées (toutes, si elle n'est pas configurée).
pub fn store_id() -> Option<u64> {
    option_env!("LUMA_STORE_ID").and_then(|v| v.trim().parse().ok())
}

/// Ce qui est enregistré sur la machine (`licence.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Premier lancement (secondes Unix) : le début de l'essai.
    pub trial_started: u64,
    pub key: Option<String>,
    /// L'activation de cette machine chez Lemon Squeezy.
    pub instance_id: Option<String>,
    pub plan: Option<Plan>,
    pub email: Option<String>,
    /// Dernière vérification réussie (secondes Unix).
    pub checked_at: u64,
}

/// Le droit d'utiliser LÜMA, maintenant.
#[derive(Clone, Debug, PartialEq)]
pub enum Access {
    /// Abonné : la formule et le courriel de l'abonnement.
    Premium { plan: Option<Plan>, email: Option<String> },
    /// Essai : jours restants (au moins 1).
    Trial { days_left: u64 },
    /// L'essai est fini, ou l'abonnement n'est plus vérifié depuis trop longtemps.
    Expired { had_key: bool },
}

impl Access {
    pub fn is_allowed(&self) -> bool {
        !matches!(self, Access::Expired { .. })
    }
}

impl State {
    /// Un état neuf : l'essai commence `now`.
    pub fn new(now: u64) -> State {
        State {
            trial_started: now,
            ..State::default()
        }
    }

    pub fn from_json(text: &str) -> Option<State> {
        serde_json::from_str(text).ok()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Le droit d'utiliser LÜMA à `now`.
    pub fn access(&self, now: u64) -> Access {
        if self.key.is_some() && self.instance_id.is_some() {
            // Une horloge reculée ne prolonge rien.
            let age = now.saturating_sub(self.checked_at);
            if self.checked_at <= now + DAY && age <= OFFLINE_GRACE_DAYS * DAY {
                return Access::Premium {
                    plan: self.plan,
                    email: self.email.clone(),
                };
            }
        }
        let used = now.saturating_sub(self.trial_started) / DAY;
        if self.trial_started <= now + DAY && used < TRIAL_DAYS {
            return Access::Trial {
                days_left: TRIAL_DAYS - used,
            };
        }
        Access::Expired {
            had_key: self.key.is_some(),
        }
    }

    /// Faut‑il revérifier la clé en ligne ?
    pub fn needs_check(&self, now: u64) -> bool {
        self.key.is_some() && now.saturating_sub(self.checked_at) >= RECHECK_HOURS * 3600
    }
}

/// Une requête à l'API de licences : POST d'un formulaire, réponse JSON.
pub trait Transport {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String>;
}

/// Les champs utiles d'une réponse de l'API (activation ou vérification).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Response {
    activated: Option<bool>,
    valid: Option<bool>,
    error: Option<String>,
    license_key: Option<LicenseKey>,
    instance: Option<Instance>,
    meta: Option<Meta>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LicenseKey {
    status: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Instance {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Meta {
    store_id: u64,
    variant_name: String,
    product_name: String,
    customer_email: String,
}

impl Meta {
    fn plan(&self) -> Plan {
        let v = format!("{} {}", self.variant_name, self.product_name).to_lowercase();
        if ["an", "annuel", "year", "annual", "yearly"]
            .iter()
            .any(|w| v.split(|c: char| !c.is_alphanumeric()).any(|t| t == *w))
        {
            Plan::Annual
        } else {
            Plan::Monthly
        }
    }
}

/// Pourquoi une clé est refusée.
#[derive(Clone, Debug, PartialEq)]
pub enum Refused {
    /// Pas de réseau (ou l'API ne répond pas) : on réessaiera.
    Offline(String),
    /// Clé inconnue, désactivée, expirée, d'une autre boutique…
    Invalid(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Offline(e) => write!(f, "impossible de joindre le serveur de licences ({e})"),
            Refused::Invalid(e) => write!(f, "{e}"),
        }
    }
}

fn parse(text: &str) -> Result<Response, Refused> {
    serde_json::from_str(text).map_err(|_| Refused::Offline("réponse illisible".into()))
}

fn check_store(r: &Response) -> Result<(), Refused> {
    match (store_id(), &r.meta) {
        (Some(ours), Some(m)) if m.store_id != ours => Err(Refused::Invalid("cette clé n'est pas une clé LÜMA".into())),
        _ => Ok(()),
    }
}

fn status_ok(r: &Response) -> Result<(), Refused> {
    match r.license_key.as_ref().map(|k| k.status.as_str()) {
        Some("expired") => Err(Refused::Invalid("l'abonnement est terminé".into())),
        Some("disabled") => Err(Refused::Invalid("cette clé a été désactivée".into())),
        _ => Ok(()),
    }
}

/// Active `key` sur cette machine (`machine` : son nom, visible dans le compte client)
/// et l'enregistre dans `state`.
pub fn activate(state: &mut State, key: &str, machine: &str, net: &dyn Transport, now: u64) -> Result<(), Refused> {
    let key = key.trim();
    if key.len() < 8 {
        return Err(Refused::Invalid("clé trop courte".into()));
    }
    let text = net
        .post(
            &format!("{API}/activate"),
            &[("license_key", key), ("instance_name", machine)],
        )
        .map_err(Refused::Offline)?;
    let r = parse(&text)?;
    if r.activated != Some(true) {
        return Err(Refused::Invalid(r.error.unwrap_or_else(|| "clé refusée".into())));
    }
    check_store(&r)?;
    status_ok(&r)?;
    let instance = r
        .instance
        .map(|i| i.id)
        .filter(|i| !i.is_empty())
        .ok_or_else(|| Refused::Invalid("activation incomplète".into()))?;
    state.key = Some(key.to_string());
    state.instance_id = Some(instance);
    state.plan = r.meta.as_ref().map(Meta::plan);
    state.email = r
        .meta
        .as_ref()
        .map(|m| m.customer_email.clone())
        .filter(|e| !e.is_empty());
    state.checked_at = now;
    Ok(())
}

/// Revérifie la clé enregistrée. Une clé refusée est retirée ; sans réseau, rien ne
/// change (la période hors ligne continue de courir).
pub fn validate(state: &mut State, net: &dyn Transport, now: u64) -> Result<(), Refused> {
    let (Some(key), Some(instance)) = (state.key.clone(), state.instance_id.clone()) else {
        return Err(Refused::Invalid("aucune clé".into()));
    };
    let text = net
        .post(
            &format!("{API}/validate"),
            &[("license_key", &key), ("instance_id", &instance)],
        )
        .map_err(Refused::Offline)?;
    let r = parse(&text)?;
    let verdict = if r.valid == Some(true) {
        check_store(&r).and_then(|()| status_ok(&r))
    } else {
        Err(Refused::Invalid(
            r.error.unwrap_or_else(|| "l'abonnement n'est plus actif".into()),
        ))
    };
    match verdict {
        Ok(()) => {
            if let Some(m) = &r.meta {
                state.plan = Some(m.plan());
                if !m.customer_email.is_empty() {
                    state.email = Some(m.customer_email.clone());
                }
            }
            state.checked_at = now;
            Ok(())
        }
        Err(e) => {
            state.key = None;
            state.instance_id = None;
            state.checked_at = 0;
            Err(e)
        }
    }
}

/// Retire la clé de cette machine (pour la passer sur une autre).
pub fn deactivate(state: &mut State, net: &dyn Transport) -> Result<(), Refused> {
    if let (Some(key), Some(instance)) = (state.key.clone(), state.instance_id.clone()) {
        net.post(
            &format!("{API}/deactivate"),
            &[("license_key", &key), ("instance_id", &instance)],
        )
        .map_err(Refused::Offline)?;
    }
    state.key = None;
    state.instance_id = None;
    state.plan = None;
    state.email = None;
    state.checked_at = 0;
    Ok(())
}

/// Un formulaire encodé pour un POST (`application/x-www-form-urlencoded`).
pub fn form_encode(form: &[(&str, &str)]) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                b' ' => "+".into(),
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    form.iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake(RefCell<Vec<Result<String, String>>>, RefCell<Vec<String>>);

    impl Fake {
        fn new(answers: Vec<Result<&str, &str>>) -> Fake {
            Fake(
                RefCell::new(
                    answers
                        .into_iter()
                        .rev()
                        .map(|a| a.map(String::from).map_err(String::from))
                        .collect(),
                ),
                RefCell::new(Vec::new()),
            )
        }
    }

    impl Transport for Fake {
        fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
            self.1.borrow_mut().push(format!("{url}?{}", form_encode(form)));
            self.0.borrow_mut().pop().unwrap_or(Err("plus de réponses".into()))
        }
    }

    const ACTIVATED: &str = r#"{"activated":true,"error":null,
        "license_key":{"id":1,"status":"active","key":"38b1460a-5104-4067-a91d-77b872934d51"},
        "instance":{"id":"47596ad9-a811-4ebf-ac8a-03fc7b6d2a17","name":"Mac de Max"},
        "meta":{"store_id":0,"product_name":"LÜMA Premium","variant_name":"Annuel","customer_email":"max@example.com"}}"#;

    #[test]
    fn a_fresh_install_gets_a_fourteen_day_trial_then_expires() {
        let s = State::new(1_000_000);
        assert_eq!(s.access(1_000_000), Access::Trial { days_left: 14 });
        assert_eq!(s.access(1_000_000 + 13 * DAY + 5), Access::Trial { days_left: 1 });
        assert_eq!(s.access(1_000_000 + 14 * DAY), Access::Expired { had_key: false });
        assert!(!s.access(1_000_000 + 30 * DAY).is_allowed());
    }

    #[test]
    fn an_activated_key_unlocks_premium_and_stays_valid_offline_for_a_while() {
        let mut s = State::new(0);
        let net = Fake::new(vec![Ok(ACTIVATED)]);
        let now = 40 * DAY;
        activate(
            &mut s,
            " 38b1460a-5104-4067-a91d-77b872934d51 ",
            "Mac de Max",
            &net,
            now,
        )
        .unwrap();
        assert!(net.1.borrow()[0].contains("instance_name=Mac+de+Max"));
        assert_eq!(
            s.access(now),
            Access::Premium {
                plan: Some(Plan::Annual),
                email: Some("max@example.com".into())
            }
        );
        // Hors ligne : toujours premium pendant la période de grâce, plus après.
        assert!(s.access(now + 13 * DAY).is_allowed());
        assert_eq!(s.access(now + 15 * DAY), Access::Expired { had_key: true });
        assert!(s.needs_check(now + DAY));
        // Survit à l'enregistrement.
        assert_eq!(State::from_json(&s.to_json()).unwrap(), s);
    }

    #[test]
    fn validation_renews_offline_time_and_a_cancelled_subscription_locks_again() {
        let mut s = State::new(0);
        let now = 40 * DAY;
        activate(&mut s, "38b1460a-5104", "Mac", &Fake::new(vec![Ok(ACTIVATED)]), now).unwrap();
        // Pas de réseau : rien ne change.
        let r = validate(&mut s, &Fake::new(vec![Err("hors ligne")]), now + 3 * DAY);
        assert!(matches!(r, Err(Refused::Offline(_))));
        assert_eq!(s.checked_at, now);
        // Abonnement renouvelé : la vérification repousse la limite.
        let ok = r#"{"valid":true,"license_key":{"status":"active"},"meta":{"variant_name":"Mensuel"}}"#;
        validate(&mut s, &Fake::new(vec![Ok(ok)]), now + 10 * DAY).unwrap();
        assert!(s.access(now + 20 * DAY).is_allowed());
        assert_eq!(s.plan, Some(Plan::Monthly));
        // Résilié : la clé expire, LÜMA se reverrouille.
        let gone = r#"{"valid":true,"license_key":{"status":"expired"},"meta":{}}"#;
        let r = validate(&mut s, &Fake::new(vec![Ok(gone)]), now + 40 * DAY);
        assert_eq!(r, Err(Refused::Invalid("l'abonnement est terminé".into())));
        assert_eq!(s.access(now + 40 * DAY), Access::Expired { had_key: false });
    }

    #[test]
    fn bad_keys_are_refused_with_the_reason() {
        let mut s = State::new(0);
        let bad = r#"{"activated":false,"error":"license_key not found.","license_key":null,"meta":null}"#;
        let r = activate(&mut s, "pas-une-vraie-cle", "Mac", &Fake::new(vec![Ok(bad)]), 10);
        assert_eq!(r, Err(Refused::Invalid("license_key not found.".into())));
        assert_eq!(s.key, None);
        assert!(matches!(
            activate(&mut s, "abc", "Mac", &Fake::new(vec![]), 10),
            Err(Refused::Invalid(_))
        ));
    }

    #[test]
    fn a_moved_back_clock_does_not_extend_anything() {
        let s = State::new(100 * DAY);
        assert_eq!(s.access(10 * DAY), Access::Expired { had_key: false });
    }

    #[test]
    fn plans_have_their_prices_and_forms_are_encoded() {
        assert_eq!(Plan::Annual.price(), "79,99 €");
        assert_eq!(form_encode(&[("a b", "é&=")]), "a+b=%C3%A9%26%3D");
    }
}
