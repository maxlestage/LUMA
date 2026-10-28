//! LÜMA Premium offert : des personnes choisies reçoivent leur clé sans payer.
//!
//! La liste est la variable `LUMA_OFFERTS` (sur Heroku) : une entrée par personne,
//! `courriel:code`, séparées par des virgules, des points‑virgules ou des retours à la
//! ligne (`alice@exemple.fr:K7Q2-PX9M, bob@exemple.fr:R4TW-8ZDN`). Le code prouve que
//! c'est bien la personne : le serveur n'envoie pas de courriel, et un courriel seul se
//! devine.
//!
//! - `/licence/offert/` : la personne tape son courriel et son code, et reçoit sa clé ;
//! - `POST /api/licence` : la clé se renouvelle (30 jours de plus) tant que le courriel
//!   est dans la liste ; retiré, l'accès s'arrête à la fin de la clé en cours.

use luma_licence::{Claims, DAY, GRACE_DAYS, Plan};

/// L'« abonnement » d'une clé offerte.
pub const SUB: &str = "offert";

/// La durée d'une clé offerte, renouvelée chaque jour par l'app.
const PERIOD_DAYS: u64 = 30;

/// La liste des accès offerts (`LUMA_OFFERTS`).
pub fn from_env() -> String {
    std::env::var("LUMA_OFFERTS").unwrap_or_default()
}

fn normalize(email: &str) -> String {
    email.trim().to_lowercase()
}

/// Les entrées (courriel, code) de la liste ; un courriel sans code ne compte pas.
fn entries(list: &str) -> impl Iterator<Item = (String, &str)> {
    list.split([',', ';', '\n'])
        .filter_map(|e| e.split_once(':'))
        .map(|(email, code)| (normalize(email), code.trim()))
        .filter(|(email, code)| !email.is_empty() && !code.is_empty())
}

/// Deux codes égaux, comparés en temps constant (la longueur mise à part).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

fn listed(list: &str, email: &str) -> bool {
    let email = normalize(email);
    entries(list).any(|(e, _)| e == email)
}

fn sign(email: &str, seed: &[u8; 32], now: u64) -> (Claims, String) {
    let claims = Claims {
        sub: SUB.to_owned(),
        email: normalize(email),
        plan: Plan::Lifetime,
        iat: now,
        exp: now + (PERIOD_DAYS + GRACE_DAYS) * DAY,
    };
    let key = luma_licence::sign(&claims, seed);
    (claims, key)
}

/// La clé de la personne qui donne `email` et `code`, s'ils sont dans la liste.
pub fn key(
    list: &str,
    seed: &[u8; 32],
    email: &str,
    code: &str,
    now: u64,
) -> Result<(Claims, String), String> {
    let (email, code) = (normalize(email), code.trim());
    if email.is_empty() || code.is_empty() {
        return Err("indiquez votre courriel et votre code".into());
    }
    // Toute la liste est parcourue : la réponse ne dit pas où la recherche s'arrête.
    let found = entries(list).fold(false, |found, (e, c)| found | (e == email && same(c, code)));
    if !found {
        return Err("ce courriel et ce code ne correspondent à aucun accès offert".into());
    }
    Ok(sign(&email, seed, now))
}

/// Une clé fraîche pour la clé offerte `key` ; `None` si ce n'est pas une clé offerte
/// (une clé d'abonnement, que Stripe renouvelle).
pub fn renew(list: &str, seed: &[u8; 32], key: &str, now: u64) -> Option<Result<String, String>> {
    let public = luma_licence::from_hex(&luma_licence::public_of(seed))?;
    let old = luma_licence::verify(key, &public).ok()?;
    if old.sub != SUB {
        return None;
    }
    Some(if listed(list, &old.email) {
        Ok(sign(&old.email, seed, now).1)
    } else {
        Err("cet accès offert a pris fin".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [7; 32];
    const LIST: &str =
        " Alice@Exemple.fr : K7Q2-PX9M ;bob@exemple.fr:R4TW-8ZDN\nsans-code@exemple.fr";

    #[test]
    fn the_right_email_and_code_give_a_key() {
        let (claims, signed) = key(LIST, &SEED, "alice@exemple.fr ", "K7Q2-PX9M", 1000).unwrap();
        assert_eq!(
            (claims.sub.as_str(), claims.email.as_str()),
            (SUB, "alice@exemple.fr")
        );
        assert_eq!(claims.exp, 1000 + (PERIOD_DAYS + GRACE_DAYS) * DAY);
        let public = luma_licence::from_hex(&luma_licence::public_of(&SEED)).unwrap();
        assert_eq!(luma_licence::verify(&signed, &public).unwrap(), claims);
        assert!(key(LIST, &SEED, "BOB@exemple.fr", "R4TW-8ZDN", 1000).is_ok());
    }

    #[test]
    fn a_wrong_code_or_an_unknown_email_gives_nothing() {
        for (email, code) in [
            ("alice@exemple.fr", "R4TW-8ZDN"),
            ("alice@exemple.fr", "k7q2-px9m"),
            ("eve@exemple.fr", "K7Q2-PX9M"),
            ("sans-code@exemple.fr", ""),
            ("", ""),
        ] {
            assert!(
                key(LIST, &SEED, email, code, 1000).is_err(),
                "{email} {code}"
            );
        }
        assert!(key("", &SEED, "alice@exemple.fr", "K7Q2-PX9M", 1000).is_err());
    }

    #[test]
    fn an_offered_key_renews_while_the_email_is_listed() {
        let (_, old) = key(LIST, &SEED, "alice@exemple.fr", "K7Q2-PX9M", 1000).unwrap();
        let fresh = renew(LIST, &SEED, &old, 2000).unwrap().unwrap();
        let public = luma_licence::from_hex(&luma_licence::public_of(&SEED)).unwrap();
        assert_eq!(luma_licence::verify(&fresh, &public).unwrap().iat, 2000);
        // Retirée de la liste : plus de renouvellement.
        assert!(
            renew("bob@exemple.fr:R4TW-8ZDN", &SEED, &old, 2000)
                .unwrap()
                .is_err()
        );
        // Une clé d'abonnement, ou une clé fausse : pas pour ce module.
        let sub = Claims {
            sub: "sub_123".into(),
            email: "alice@exemple.fr".into(),
            plan: Plan::Monthly,
            iat: 1000,
            exp: 9000,
        };
        assert!(renew(LIST, &SEED, &luma_licence::sign(&sub, &SEED), 2000).is_none());
        assert!(renew(LIST, &SEED, "LUMA1.abc", 2000).is_none());
    }
}
