//! LÜMA Premium : 14 jours d'essai, puis un abonnement, au mois ou à l'an.
//!
//! Le paiement passe par Stripe (liens de paiement). Après le paiement, Stripe renvoie
//! le client sur le site de LÜMA, dont le serveur vérifie l'abonnement auprès de Stripe
//! et délivre une **clé de licence signée** (Ed25519) : qui est abonné, à quelle
//! formule, et jusqu'à quand (fin de la période payée, plus [`GRACE_DAYS`] jours).
//!
//! L'app vérifie la clé elle‑même, hors ligne, avec la clé publique du serveur
//! (`LUMA_LICENCE_PUBLIC`, à la compilation) : aucune base de données, aucune
//! bibliothèque TLS, et la même clé marche sur Mac, iPhone et iPad. Une fois par jour,
//! en ligne, l'app demande au serveur une clé fraîche (`/api/licence`) : il la délivre
//! tant que l'abonnement est actif chez Stripe. Résilié, l'abonnement court jusqu'au
//! bout de la période payée, puis la clé expire.
//!
//! Ce module ne fait pas de réseau : il signe et vérifie les clés, décide (essai,
//! abonné, expiré) et dit quoi demander. L'app fournit le transport ([`Transport`]).
//!
//! Sur iPhone et iPad, l'abonnement s'achète uniquement dans l'App Store (achats
//! intégrés, `luma_apple::store`) : l'app enregistre chaque achat ([`record_store`]) et
//! seul cet abonnement y ouvre LÜMA Premium ; les clés y sont ignorées. Sur Mac, c'est
//! l'inverse : seulement la clé (Stripe).
//!
//! Configuration à la compilation, sans toucher au code : `LUMA_CHECKOUT_MONTHLY`,
//! `LUMA_CHECKOUT_ANNUAL` (liens de paiement Stripe), `LUMA_BILLING_PORTAL` (portail
//! client Stripe), `LUMA_LICENCE_PUBLIC` (clé publique, 64 chiffres hexadécimaux) et
//! `LUMA_LICENCE_SERVER` (le site, ex. `https://luma.app`), et les produits de l'App
//! Store `LUMA_APPSTORE_MONTHLY`, `LUMA_APPSTORE_ANNUAL` (par défaut
//! `app.luma.studio.premium.monthly` et `….annual`).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Durée de l'essai gratuit.
pub const TRIAL_DAYS: u64 = 14;
/// Les jours ajoutés après la fin de la période payée : le temps d'un renouvellement,
/// ou d'un voyage sans réseau.
pub const GRACE_DAYS: u64 = 14;
/// Demande d'une clé fraîche au plus une fois par…
pub const RECHECK_HOURS: u64 = 24;

pub const DAY: u64 = 86_400;

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
            Plan::Monthly => "19,99 €",
            Plan::Annual => "239,88 €",
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
            Plan::Annual => "Soit 19,99 € par mois, en un seul paiement.",
        }
    }

    /// Le lien de paiement Stripe, s'il est configuré pour cette version.
    pub fn checkout_url(self) -> Option<&'static str> {
        let url = match self {
            Plan::Monthly => option_env!("LUMA_CHECKOUT_MONTHLY"),
            Plan::Annual => option_env!("LUMA_CHECKOUT_ANNUAL"),
        };
        url.filter(|u| u.starts_with("https://"))
    }

    /// L'identifiant du produit dans l'App Store (App Store Connect).
    pub fn app_store_id(self) -> &'static str {
        match self {
            Plan::Monthly => option_env!("LUMA_APPSTORE_MONTHLY").unwrap_or("app.luma.studio.premium.monthly"),
            Plan::Annual => option_env!("LUMA_APPSTORE_ANNUAL").unwrap_or("app.luma.studio.premium.annual"),
        }
    }

    /// La formule d'un produit de l'App Store.
    pub fn from_app_store_id(id: &str) -> Option<Plan> {
        Plan::ALL.into_iter().find(|p| p.app_store_id() == id)
    }

    /// La durée d'une période payée, en secondes (un mois de 31 jours, une année de
    /// 366 : jamais plus court que la vraie).
    pub fn period_secs(self) -> u64 {
        match self {
            Plan::Monthly => 31 * DAY,
            Plan::Annual => 366 * DAY,
        }
    }

    /// La formule d'un intervalle de facturation Stripe (`month`, `year`).
    pub fn from_interval(interval: &str) -> Plan {
        if interval == "year" {
            Plan::Annual
        } else {
            Plan::Monthly
        }
    }
}

/// Le portail client Stripe (gérer, résilier, factures), s'il est configuré.
pub fn billing_portal() -> Option<&'static str> {
    option_env!("LUMA_BILLING_PORTAL").filter(|u| u.starts_with("https://"))
}

/// Le serveur des clés (le site), s'il est configuré.
pub fn licence_server() -> Option<&'static str> {
    option_env!("LUMA_LICENCE_SERVER")
        .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
        .map(|u| u.trim_end_matches('/'))
}

/// La clé publique qui vérifie les licences, si elle est configurée.
pub fn public_key() -> Option<[u8; 32]> {
    option_env!("LUMA_LICENCE_PUBLIC").and_then(from_hex)
}

// ----- Clés de licence ------------------------------------------------------------------

/// Ce que dit une clé de licence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// L'abonnement Stripe (`sub_…`) : c'est lui que le serveur revérifie.
    pub sub: String,
    pub email: String,
    pub plan: Plan,
    /// Délivrée le, valable jusqu'au (secondes Unix).
    pub iat: u64,
    pub exp: u64,
}

const PREFIX: &str = "LUMA1.";

/// Signe une clé de licence avec la clé privée du serveur (`seed`, 32 octets).
pub fn sign(claims: &Claims, seed: &[u8; 32]) -> String {
    let payload = serde_json::to_vec(claims).unwrap_or_default();
    let signature = SigningKey::from_bytes(seed).sign(&payload);
    format!("{PREFIX}{}.{}", b64(&payload), b64(&signature.to_bytes()))
}

/// Vérifie une clé de licence (signature seule ; l'expiration est vérifiée par
/// [`State::access`]).
pub fn verify(key: &str, public: &[u8; 32]) -> Result<Claims, String> {
    let rest = key.trim().strip_prefix(PREFIX).ok_or("ce n'est pas une clé LÜMA")?;
    let (payload, signature) = rest.split_once('.').ok_or("clé incomplète")?;
    let payload = unb64(payload).ok_or("clé illisible")?;
    let signature: [u8; 64] = unb64(signature)
        .and_then(|s| s.try_into().ok())
        .ok_or("signature illisible")?;
    let key = VerifyingKey::from_bytes(public).map_err(|_| "clé publique invalide")?;
    key.verify(&payload, &Signature::from_bytes(&signature))
        .map_err(|_| "clé non reconnue (signature fausse)")?;
    serde_json::from_slice(&payload).map_err(|_| "clé illisible".to_owned())
}

/// La clé publique (64 chiffres hexadécimaux) d'une clé privée.
pub fn public_of(seed: &[u8; 32]) -> String {
    hex(SigningKey::from_bytes(seed).verifying_key().as_bytes())
}

/// Une nouvelle paire de clés, depuis 32 octets aléatoires : (privée, publique), en
/// hexadécimal.
pub fn keypair(random: [u8; 32]) -> (String, String) {
    (hex(&random), public_of(&random))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64 « URL » sans remplissage.
fn b64(data: &[u8]) -> String {
    let mut out = String::new();
    for c in data.chunks(3) {
        let v =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..=c.len() {
            out.push(B64[(v >> (18 - 6 * k) & 63) as usize] as char);
        }
    }
    out
}

fn unb64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for ch in s.bytes() {
        let v = B64.iter().position(|&b| b == ch)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ----- Ce que l'app garde ------------------------------------------------------------

/// Ce qui est enregistré sur la machine (`licence.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Premier lancement (secondes Unix) : le début de l'essai.
    pub trial_started: u64,
    pub key: Option<String>,
    /// Dernière demande d'une clé fraîche (secondes Unix).
    pub checked_at: u64,
    /// L'abonnement acheté dans l'App Store (iPhone et iPad).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<StoreSubscription>,
}

/// Un abonnement de l'App Store : la formule, et la fin de validité (dernier achat ou
/// renouvellement, plus une période, plus [`GRACE_DAYS`] jours). Chaque renouvellement
/// arrive à l'app comme un nouvel achat et repousse la fin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreSubscription {
    pub plan: Plan,
    pub until: u64,
}

/// Les clés de licence ouvrent‑elles LÜMA ici ? Sur Mac (et ailleurs), oui ; sur iPhone
/// et iPad, non : seul l'App Store y vend l'abonnement.
pub const KEYS_ALLOWED: bool = !cfg!(target_os = "ios");

/// Enregistre un achat (ou un renouvellement, une restauration) de l'App Store : le
/// produit `product`, payé à `date` (secondes Unix). Un produit inconnu ne change rien.
/// Rend `true` si l'abonnement s'allonge.
pub fn record_store(state: &mut State, product: &str, date: u64) -> bool {
    let Some(plan) = Plan::from_app_store_id(product) else {
        return false;
    };
    let until = date + plan.period_secs() + GRACE_DAYS * DAY;
    if state.store.as_ref().is_some_and(|s| s.until >= until) {
        return false;
    }
    state.store = Some(StoreSubscription { plan, until });
    true
}

/// Le droit d'utiliser LÜMA, maintenant.
#[derive(Clone, Debug, PartialEq)]
pub enum Access {
    /// Abonné : la formule, le courriel, et la fin de validité de la clé.
    Premium {
        plan: Option<Plan>,
        email: Option<String>,
        until: u64,
    },
    /// Essai : jours restants (au moins 1).
    Trial { days_left: u64 },
    /// L'essai est fini, ou l'abonnement aussi.
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

    /// Les informations de la clé enregistrée, si elle est authentique.
    pub fn claims(&self, public: Option<&[u8; 32]>) -> Option<Claims> {
        verify(self.key.as_deref()?, public?).ok()
    }

    /// Le droit d'utiliser LÜMA à `now`, avec la clé publique `public` (les clés ne
    /// comptent que là où elles sont vendues : [`KEYS_ALLOWED`]).
    pub fn access(&self, now: u64, public: Option<&[u8; 32]>) -> Access {
        self.access_with(now, public, KEYS_ALLOWED)
    }

    /// [`State::access`], en disant si les clés comptent (`keys`) ; l'abonnement de
    /// l'App Store compte toujours.
    pub fn access_with(&self, now: u64, public: Option<&[u8; 32]>, keys: bool) -> Access {
        if let Some(s) = &self.store
            && now < s.until
        {
            return Access::Premium {
                plan: Some(s.plan),
                email: None,
                until: s.until,
            };
        }
        if keys
            && let Some(c) = self.claims(public)
            && now < c.exp
            && c.iat <= now + DAY
        {
            return Access::Premium {
                plan: Some(c.plan),
                email: (!c.email.is_empty()).then(|| c.email.clone()),
                until: c.exp,
            };
        }
        let used = now.saturating_sub(self.trial_started) / DAY;
        // Une horloge reculée ne prolonge rien.
        if self.trial_started <= now + DAY && used < TRIAL_DAYS {
            return Access::Trial {
                days_left: TRIAL_DAYS - used,
            };
        }
        Access::Expired {
            had_key: (keys && self.key.is_some()) || self.store.is_some(),
        }
    }

    /// Faut‑il demander une clé fraîche ?
    pub fn needs_check(&self, now: u64) -> bool {
        KEYS_ALLOWED && self.key.is_some() && now.saturating_sub(self.checked_at) >= RECHECK_HOURS * 3600
    }
}

/// Une requête au serveur des clés : POST d'un formulaire, réponse JSON.
pub trait Transport {
    fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String>;
}

/// Pourquoi une clé est refusée.
#[derive(Clone, Debug, PartialEq)]
pub enum Refused {
    /// Pas de réseau (ou le serveur ne répond pas) : on réessaiera.
    Offline(String),
    /// Clé fausse, abonnement terminé…
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

/// Enregistre une clé saisie, après l'avoir vérifiée (hors ligne).
pub fn activate(state: &mut State, key: &str, public: Option<&[u8; 32]>, now: u64) -> Result<Claims, Refused> {
    let public =
        public.ok_or_else(|| Refused::Invalid("cette version de LÜMA ne vérifie pas encore les clés".into()))?;
    let claims = verify(key, public).map_err(Refused::Invalid)?;
    if claims.exp <= now {
        return Err(Refused::Invalid(
            "cette clé a expiré : l'app en demande une neuve en ligne".into(),
        ));
    }
    state.key = Some(key.trim().to_owned());
    state.checked_at = now;
    Ok(claims)
}

/// Demande au serveur une clé fraîche pour l'abonnement en cours. Sans réseau, ou si
/// l'abonnement est terminé, la clé enregistrée reste (valable jusqu'à son terme).
pub fn refresh(
    state: &mut State,
    server: &str,
    public: Option<&[u8; 32]>,
    net: &dyn Transport,
    now: u64,
) -> Result<(), Refused> {
    let key = state.key.clone().ok_or_else(|| Refused::Invalid("aucune clé".into()))?;
    let text = net
        .post(&format!("{server}/api/licence"), &[("key", &key)])
        .map_err(Refused::Offline)?;
    #[derive(Deserialize)]
    struct Answer {
        key: Option<String>,
        error: Option<String>,
    }
    let answer: Answer = serde_json::from_str(&text).map_err(|_| Refused::Offline("réponse illisible".into()))?;
    state.checked_at = now;
    match (answer.key, answer.error) {
        (Some(fresh), _) => {
            activate(state, &fresh, public, now)?;
            Ok(())
        }
        (None, e) => Err(Refused::Invalid(e.unwrap_or_else(|| "abonnement inactif".into()))),
    }
}

/// Retire la clé de cette machine.
pub fn forget(state: &mut State) {
    state.key = None;
    state.checked_at = 0;
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

    const SEED: [u8; 32] = [7; 32];

    fn public() -> [u8; 32] {
        from_hex(&public_of(&SEED)).unwrap()
    }

    fn claims(exp: u64) -> Claims {
        Claims {
            sub: "sub_123".into(),
            email: "max@example.com".into(),
            plan: Plan::Annual,
            iat: 0,
            exp,
        }
    }

    struct Fake(RefCell<Vec<Result<String, String>>>);

    impl Transport for Fake {
        fn post(&self, url: &str, form: &[(&str, &str)]) -> Result<String, String> {
            assert!(url.ends_with("/api/licence") && form[0].0 == "key");
            self.0.borrow_mut().pop().unwrap_or(Err("plus de réponses".into()))
        }
    }

    #[test]
    fn keys_are_signed_and_any_change_is_refused() {
        let key = sign(&claims(1000), &SEED);
        assert!(key.starts_with("LUMA1."));
        assert_eq!(verify(&key, &public()).unwrap(), claims(1000));
        // Une lettre changée dans les informations, ou une autre clé publique : refusée.
        let (head, sig) = key.rsplit_once('.').unwrap();
        let forged = format!("{}x.{sig}", &head[..head.len() - 1]);
        assert!(verify(&forged, &public()).is_err());
        let other = from_hex(&public_of(&[9; 32])).unwrap();
        assert!(verify(&key, &other).is_err());
        assert!(verify("pas une clé", &public()).is_err());
        for n in 0..10 {
            let data: Vec<u8> = (0..n).collect();
            assert_eq!(unb64(&b64(&data)).unwrap(), data);
        }
    }

    #[test]
    fn a_fresh_install_gets_a_fourteen_day_trial_then_expires() {
        let s = State::new(1_000_000);
        let p = public();
        assert_eq!(s.access(1_000_000, Some(&p)), Access::Trial { days_left: 14 });
        assert_eq!(
            s.access(1_000_000 + 13 * DAY + 5, Some(&p)),
            Access::Trial { days_left: 1 }
        );
        assert_eq!(
            s.access(1_000_000 + 14 * DAY, Some(&p)),
            Access::Expired { had_key: false }
        );
        // Une horloge reculée ne prolonge rien.
        assert!(!State::new(100 * DAY).access(10 * DAY, Some(&p)).is_allowed());
    }

    #[test]
    fn a_key_unlocks_premium_until_it_expires_and_refreshes_while_subscribed() {
        let p = public();
        let now = 40 * DAY;
        let mut s = State::new(0);
        let key = sign(&claims(now + 30 * DAY), &SEED);
        activate(&mut s, &key, Some(&p), now).unwrap();
        assert!(matches!(
            s.access(now, Some(&p)),
            Access::Premium {
                plan: Some(Plan::Annual),
                ..
            }
        ));
        assert!(s.access(now + 29 * DAY, Some(&p)).is_allowed());
        assert_eq!(s.access(now + 31 * DAY, Some(&p)), Access::Expired { had_key: true });
        // Renouvelé : le serveur rend une clé plus longue.
        let fresh = sign(&claims(now + 60 * DAY), &SEED);
        let net = Fake(RefCell::new(vec![Ok(format!(r#"{{"key":"{fresh}"}}"#))]));
        refresh(&mut s, "https://luma.app", Some(&p), &net, now + 20 * DAY).unwrap();
        assert!(s.access(now + 50 * DAY, Some(&p)).is_allowed());
        // Résilié : la clé reste jusqu'à son terme.
        let net = Fake(RefCell::new(vec![Ok(r#"{"error":"abonnement terminé"}"#.into())]));
        let r = refresh(&mut s, "https://luma.app", Some(&p), &net, now + 40 * DAY);
        assert_eq!(r, Err(Refused::Invalid("abonnement terminé".into())));
        assert!(s.access(now + 50 * DAY, Some(&p)).is_allowed());
        assert!(!s.access(now + 61 * DAY, Some(&p)).is_allowed());
        // Hors ligne : rien ne change.
        let net = Fake(RefCell::new(vec![Err("hors ligne".into())]));
        assert!(matches!(
            refresh(&mut s, "https://luma.app", Some(&p), &net, now),
            Err(Refused::Offline(_))
        ));
        assert_eq!(State::from_json(&s.to_json()).unwrap(), s);
    }

    #[test]
    fn forged_or_expired_keys_are_refused_with_the_reason() {
        let p = public();
        let mut s = State::new(0);
        let bad = sign(&claims(10 * DAY), &[1; 32]);
        assert!(matches!(activate(&mut s, &bad, Some(&p), 0), Err(Refused::Invalid(_))));
        let old = sign(&claims(10), &SEED);
        assert!(matches!(
            activate(&mut s, &old, Some(&p), 100),
            Err(Refused::Invalid(_))
        ));
        assert!(matches!(activate(&mut s, &old, None, 0), Err(Refused::Invalid(_))));
        assert_eq!(s.key, None);
    }

    #[test]
    fn app_store_purchases_open_premium_and_renewals_extend_it() {
        let now = 1_800_000_000;
        let mut s = State::new(now - 30 * DAY);
        assert!(matches!(
            s.access_with(now, None, false),
            Access::Expired { had_key: false }
        ));
        assert!(!record_store(&mut s, "autre.produit", now), "produit inconnu");
        let monthly = Plan::Monthly.app_store_id();
        assert!(record_store(&mut s, monthly, now));
        let until = now + 31 * DAY + GRACE_DAYS * DAY;
        assert_eq!(
            s.access_with(now, None, false),
            Access::Premium {
                plan: Some(Plan::Monthly),
                email: None,
                until
            }
        );
        // Un achat plus ancien (une restauration) ne raccourcit rien ; un renouvellement
        // allonge.
        assert!(!record_store(&mut s, monthly, now - 40 * DAY));
        assert!(record_store(&mut s, monthly, now + 31 * DAY));
        assert!(s.access_with(until + DAY, None, false).is_allowed());
        // Passé la fin, l'essai ne revient pas.
        assert!(matches!(
            s.access_with(now + 200 * DAY, None, false),
            Access::Expired { had_key: true }
        ));
        // L'état garde l'abonnement.
        assert_eq!(State::from_json(&s.to_json()), Some(s.clone()));
    }

    #[test]
    fn keys_count_only_where_they_are_sold() {
        let (secret, public) = keypair([7; 32]);
        let (seed, public) = (from_hex(&secret).unwrap(), from_hex(&public).unwrap());
        let now = 1_800_000_000;
        let claims = Claims {
            sub: "sub_1".into(),
            email: "a@b.c".into(),
            plan: Plan::Annual,
            iat: now,
            exp: now + 400 * DAY,
        };
        let mut s = State::new(now - 30 * DAY);
        s.key = Some(sign(&claims, &seed));
        assert!(matches!(
            s.access_with(now, Some(&public), true),
            Access::Premium { .. }
        ));
        // Sur iPhone et iPad, une clé n'ouvre rien.
        assert!(matches!(
            s.access_with(now, Some(&public), false),
            Access::Expired { .. }
        ));
    }

    #[test]
    fn plans_have_their_prices_and_forms_are_encoded() {
        assert_eq!(Plan::Annual.price(), "239,88 €");
        assert_eq!(Plan::from_interval("year"), Plan::Annual);
        assert_eq!(form_encode(&[("a b", "é&=")]), "a+b=%C3%A9%26%3D");
        let (secret, public) = keypair([3; 32]);
        assert_eq!(public_of(&from_hex(&secret).unwrap()), public);
    }
}
