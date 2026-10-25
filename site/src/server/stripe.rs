//! LÜMA Premium côté serveur : Stripe encaisse, ce serveur délivre les clés de licence.
//!
//! - `GET /licence/?session_id=cs_…` : la page où Stripe renvoie le client après le
//!   paiement (lien de paiement → « Après le paiement » → cette adresse avec
//!   `{CHECKOUT_SESSION_ID}`). Le serveur relit la session chez Stripe, vérifie
//!   l'abonnement, et affiche la clé signée. Le lien reste valable : le revisiter
//!   redonne une clé à jour.
//! - `POST /api/licence` (`key=…`) : l'app y demande une clé fraîche une fois par jour ;
//!   elle est délivrée tant que l'abonnement est actif chez Stripe.
//!
//! Pas de base de données : Stripe est la référence, la clé porte l'abonnement. Les
//! appels à Stripe passent par `curl` (présent sur Heroku), la clé secrète par l'entrée
//! standard, jamais dans la ligne de commande.
//!
//! Variables : `STRIPE_SECRET_KEY` (sk_live_… ou sk_test_…), `LUMA_LICENCE_SECRET` (la
//! clé privée de signature, 64 chiffres hexadécimaux, `luma licence-keys`) et
//! `LUMA_BILLING_PORTAL` (le portail client Stripe).

use luma_licence::{Claims, DAY, GRACE_DAYS, Plan};
use serde_json::Value;

/// Ce qui peut lire Stripe (le vrai, ou un faux pour les tests).
pub trait Api {
    fn get(&self, path: &str) -> Result<Value, String>;
}

/// L'API de Stripe, par `curl`.
pub struct Stripe {
    secret: String,
}

impl Stripe {
    /// Stripe, si `STRIPE_SECRET_KEY` est configurée.
    pub fn from_env() -> Option<Stripe> {
        std::env::var("STRIPE_SECRET_KEY")
            .ok()
            .filter(|k| k.starts_with("sk_") || k.starts_with("rk_"))
            .map(|secret| Stripe { secret })
    }
}

impl Api for Stripe {
    fn get(&self, path: &str) -> Result<Value, String> {
        use std::io::Write;
        let mut child = std::process::Command::new("curl")
            .args(["-sS", "--max-time", "20", "-K", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("curl : {e}"))?;
        let config = format!(
            "url = \"https://api.stripe.com/v1/{path}\"\nuser = \"{}:\"\n",
            self.secret.replace('"', "")
        );
        child
            .stdin
            .take()
            .ok_or("curl : entrée")?
            .write_all(config.as_bytes())
            .map_err(|e| format!("curl : {e}"))?;
        let out = child
            .wait_with_output()
            .map_err(|e| format!("curl : {e}"))?;
        let value: Value = serde_json::from_slice(&out.stdout).map_err(|_| {
            format!(
                "Stripe ne répond pas ({})",
                String::from_utf8_lossy(&out.stderr).trim()
            )
        })?;
        match value.get("error") {
            Some(e) => Err(e["message"].as_str().unwrap_or("erreur Stripe").to_owned()),
            None => Ok(value),
        }
    }
}

/// La clé privée de signature (`LUMA_LICENCE_SECRET`).
pub fn signing_key() -> Option<[u8; 32]> {
    std::env::var("LUMA_LICENCE_SECRET")
        .ok()
        .and_then(|s| luma_licence::from_hex(&s))
}

/// Le portail client Stripe, s'il est configuré.
pub fn billing_portal() -> Option<String> {
    std::env::var("LUMA_BILLING_PORTAL")
        .ok()
        .filter(|u| u.starts_with("https://"))
}

/// Un abonnement actif ? (`active`, `trialing` ; `past_due` : le paiement est en cours
/// de nouvelle tentative, on laisse la période de grâce jouer.)
fn active(sub: &Value) -> bool {
    matches!(
        sub["status"].as_str(),
        Some("active" | "trialing" | "past_due")
    )
}

/// La fin de la période payée : à la racine de l'abonnement (anciennes versions de
/// l'API) ou sur son premier article (versions récentes).
fn period_end(sub: &Value) -> Option<u64> {
    sub["current_period_end"]
        .as_u64()
        .or_else(|| sub["items"]["data"][0]["current_period_end"].as_u64())
}

fn plan(sub: &Value) -> Plan {
    let item = &sub["items"]["data"][0];
    let interval = item["price"]["recurring"]["interval"]
        .as_str()
        .or_else(|| item["plan"]["interval"].as_str())
        .unwrap_or("month");
    Plan::from_interval(interval)
}

/// La clé d'un abonnement Stripe (`sub`, l'objet abonnement), pour `email`.
fn key_for(
    sub: &Value,
    email: &str,
    seed: &[u8; 32],
    now: u64,
) -> Result<(Claims, String), String> {
    if !active(sub) {
        return Err("l'abonnement n'est plus actif".into());
    }
    let end = period_end(sub).ok_or("abonnement sans période")?;
    let claims = Claims {
        sub: sub["id"]
            .as_str()
            .ok_or("abonnement sans identifiant")?
            .to_owned(),
        email: email.to_owned(),
        plan: plan(sub),
        iat: now,
        exp: end.max(now) + GRACE_DAYS * DAY,
    };
    let key = luma_licence::sign(&claims, seed);
    Ok((claims, key))
}

/// La clé d'une session de paiement terminée (`cs_…`).
pub fn key_for_session(
    api: &dyn Api,
    seed: &[u8; 32],
    session: &str,
    now: u64,
) -> Result<(Claims, String), String> {
    if !session.starts_with("cs_")
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("lien de paiement invalide".into());
    }
    let s = api.get(&format!(
        "checkout/sessions/{session}?expand[]=subscription"
    ))?;
    if s["status"].as_str() != Some("complete") {
        return Err("le paiement n'est pas terminé".into());
    }
    let email = s["customer_details"]["email"]
        .as_str()
        .or_else(|| s["customer_email"].as_str())
        .unwrap_or("");
    let sub = match &s["subscription"] {
        Value::String(id) => api.get(&format!("subscriptions/{id}"))?,
        Value::Object(_) => s["subscription"].clone(),
        _ => return Err("ce paiement n'est pas un abonnement LÜMA".into()),
    };
    key_for(&sub, email, seed, now)
}

/// Une clé fraîche pour la clé `key` (délivrée par ce serveur), si l'abonnement court.
pub fn refresh(api: &dyn Api, seed: &[u8; 32], key: &str, now: u64) -> Result<String, String> {
    let public = luma_licence::from_hex(&luma_licence::public_of(seed)).ok_or("clé publique")?;
    let old = luma_licence::verify(key, &public)?;
    if !old.sub.starts_with("sub_")
        || !old
            .sub
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("clé sans abonnement".into());
    }
    let sub = api.get(&format!("subscriptions/{}", old.sub))?;
    key_for(&sub, &old.email, seed, now).map(|(_, k)| k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fake(Vec<(&'static str, Value)>);

    impl Api for Fake {
        fn get(&self, path: &str) -> Result<Value, String> {
            self.0
                .iter()
                .find(|(p, _)| path.starts_with(p))
                .map(|(_, v)| v.clone())
                .ok_or_else(|| format!("inconnu : {path}"))
        }
    }

    const SEED: [u8; 32] = [5; 32];

    fn sub(status: &str, end: u64, interval: &str) -> Value {
        json!({"id": "sub_1Abc", "status": status,
               "items": {"data": [{"current_period_end": end, "price": {"recurring": {"interval": interval}}}]}})
    }

    #[test]
    fn a_completed_checkout_gives_a_signed_key_until_the_end_of_the_period_plus_grace() {
        let api = Fake(vec![(
            "checkout/sessions/cs_test_1",
            json!({"status": "complete", "customer_details": {"email": "max@example.com"},
                   "subscription": sub("active", 1_000_000, "year")}),
        )]);
        let (claims, key) = key_for_session(&api, &SEED, "cs_test_1", 500_000).unwrap();
        assert_eq!(claims.plan, Plan::Annual);
        assert_eq!(claims.exp, 1_000_000 + GRACE_DAYS * DAY);
        let public = luma_licence::from_hex(&luma_licence::public_of(&SEED)).unwrap();
        assert_eq!(luma_licence::verify(&key, &public).unwrap(), claims);
        // Une session inachevée, ou un identifiant bizarre : pas de clé.
        let open = Fake(vec![("checkout/sessions/cs_x", json!({"status": "open"}))]);
        assert!(key_for_session(&open, &SEED, "cs_x", 0).is_err());
        assert!(key_for_session(&api, &SEED, "cs_../../charges", 0).is_err());
    }

    #[test]
    fn keys_are_refreshed_while_the_subscription_runs_and_not_after() {
        let seed = SEED;
        let first = luma_licence::sign(
            &Claims {
                sub: "sub_1Abc".into(),
                email: "max@example.com".into(),
                plan: Plan::Monthly,
                iat: 0,
                exp: 100,
            },
            &seed,
        );
        let renewed = Fake(vec![(
            "subscriptions/sub_1Abc",
            sub("active", 5_000_000, "month"),
        )]);
        let fresh = refresh(&renewed, &seed, &first, 4_000_000).unwrap();
        let public = luma_licence::from_hex(&luma_licence::public_of(&seed)).unwrap();
        assert_eq!(
            luma_licence::verify(&fresh, &public).unwrap().exp,
            5_000_000 + GRACE_DAYS * DAY
        );
        let cancelled = Fake(vec![(
            "subscriptions/sub_1Abc",
            sub("canceled", 5_000_000, "month"),
        )]);
        assert!(refresh(&cancelled, &seed, &first, 4_000_000).is_err());
        // Une clé signée par quelqu'un d'autre : refusée sans même demander à Stripe.
        let forged = luma_licence::sign(&luma_licence::verify(&first, &public).unwrap(), &[1; 32]);
        assert!(refresh(&renewed, &seed, &forged, 0).is_err());
    }
}
