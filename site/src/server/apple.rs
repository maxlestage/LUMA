//! « Se connecter avec Apple » : le jeton d'identité que l'app reçoit d'Apple sur
//! iPhone et iPad, vérifié ici avant de délivrer une clé offerte.
//!
//! Le jeton est un JWT signé par Apple (RS256) : sa signature est vérifiée avec les
//! clés publiques d'Apple (`https://appleid.apple.com/auth/keys`, lues par `curl` et
//! gardées une heure), puis son émetteur, son destinataire (l'app, `app.luma.studio`,
//! ou `LUMA_APPLE_AUDIENCE`), sa date et son courriel vérifié. Le courriel ainsi prouvé
//! remplace le code de `/licence/offert/` : il suffit qu'il soit dans la liste.

use std::sync::Mutex;

use num_bigint::BigUint;
use serde_json::Value;
use sha2::{Digest, Sha256};

const ISSUER: &str = "https://appleid.apple.com";
const KEYS_URL: &str = "https://appleid.apple.com/auth/keys";

/// L'identifiant de l'app, destinataire attendu du jeton.
pub fn audience() -> String {
    std::env::var("LUMA_APPLE_AUDIENCE").unwrap_or_else(|_| "app.luma.studio".into())
}

/// Le courriel du compte Apple, si `token` est un jeton d'identité authentique, émis
/// pour l'app, encore valable, et dont le courriel est vérifié.
pub fn verified_email(token: &str, now: u64) -> Result<String, String> {
    let kid = header(token)?;
    let mut keys = apple_keys(false)?;
    if find_key(&keys, &kid).is_none() {
        // Apple a peut‑être changé de clés depuis la dernière lecture.
        keys = apple_keys(true)?;
    }
    verify_with(token, &keys, &audience(), now)
}

/// Les clés publiques d'Apple (le JSON), lues au plus une fois par heure, ou tout de
/// suite si `fresh`.
fn apple_keys(fresh: bool) -> Result<String, String> {
    static CACHE: Mutex<Option<(std::time::Instant, String)>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if !fresh
        && let Some((at, keys)) = cache.as_ref()
        && at.elapsed() < std::time::Duration::from_secs(3600)
    {
        return Ok(keys.clone());
    }
    let out = std::process::Command::new("curl")
        .args(["-sS", "--fail", "--max-time", "10", KEYS_URL])
        .output()
        .map_err(|e| format!("curl : {e}"))?;
    let keys = String::from_utf8(out.stdout).map_err(|_| "clés d'Apple illisibles".to_owned())?;
    if !out.status.success() || serde_json::from_str::<Value>(&keys).is_err() {
        return Err("impossible de lire les clés d'Apple".into());
    }
    *cache = Some((std::time::Instant::now(), keys.clone()));
    Ok(keys)
}

/// Le `kid` de l'en‑tête (la clé d'Apple qui a signé), si l'algorithme est RS256.
fn header(token: &str) -> Result<String, String> {
    let head = token.split('.').next().unwrap_or_default();
    let head: Value =
        serde_json::from_slice(&base64url(head)?).map_err(|_| "jeton illisible".to_owned())?;
    if head["alg"] != "RS256" {
        return Err("jeton refusé (algorithme)".into());
    }
    head["kid"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "jeton illisible".into())
}

/// La clé `kid` des clés publiques `keys` (un JWKS) : (module, exposant).
fn find_key(keys: &str, kid: &str) -> Option<(BigUint, BigUint)> {
    let keys: Value = serde_json::from_str(keys).ok()?;
    let key = keys["keys"]
        .as_array()?
        .iter()
        .find(|k| k["kid"] == kid && k["kty"] == "RSA")?;
    let n = base64url(key["n"].as_str()?).ok()?;
    let e = base64url(key["e"].as_str()?).ok()?;
    Some((BigUint::from_bytes_be(&n), BigUint::from_bytes_be(&e)))
}

/// [`verified_email`] avec les clés publiques `keys` et le destinataire `audience`.
fn verify_with(token: &str, keys: &str, audience: &str, now: u64) -> Result<String, String> {
    let parts: Vec<&str> = token.trim().split('.').collect();
    let [head, body, signature] = parts[..] else {
        return Err("jeton illisible".into());
    };
    let kid = header(token.trim())?;
    let (n, e) = find_key(keys, &kid).ok_or("jeton refusé (clé inconnue)")?;
    let signed = format!("{head}.{body}");
    if !rs256(&n, &e, signed.as_bytes(), &base64url(signature)?) {
        return Err("jeton refusé (signature)".into());
    }
    let claims: Value =
        serde_json::from_slice(&base64url(body)?).map_err(|_| "jeton illisible".to_owned())?;
    check(&claims, audience, now)
}

/// Les informations du jeton : émetteur, destinataire, dates, courriel vérifié.
fn check(claims: &Value, audience: &str, now: u64) -> Result<String, String> {
    let audience_ok = match &claims["aud"] {
        Value::String(a) => a == audience,
        Value::Array(list) => list.iter().any(|a| a == audience),
        _ => false,
    };
    if claims["iss"] != ISSUER || !audience_ok {
        return Err("jeton refusé (il n'est pas pour LÜMA)".into());
    }
    let exp = claims["exp"].as_u64().unwrap_or(0);
    let iat = claims["iat"].as_u64().unwrap_or(u64::MAX);
    if now >= exp || iat > now + 300 {
        return Err("jeton expiré : reconnectez‑vous avec Apple".into());
    }
    // Apple écrit le booléen en texte ("true") ou en JSON (true).
    let yes = |v: &Value| v == true || v == "true";
    let email = claims["email"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if email.is_empty() || !yes(&claims["email_verified"]) {
        return Err("partagez votre adresse e‑mail avec LÜMA pour recevoir votre accès".into());
    }
    if yes(&claims["is_private_email"]) {
        return Err(
            "votre adresse e‑mail est masquée : dans Réglages › Identifiant Apple › Se connecter avec Apple, retirez LÜMA, puis reconnectez‑vous en partageant votre adresse".into(),
        );
    }
    Ok(email)
}

/// La signature RSASSA‑PKCS1‑v1_5 avec SHA‑256 (RS256) : `signature`^e mod n doit être
/// l'empreinte du message, encodée comme le veut la norme (RFC 8017, § 8.2.2).
fn rs256(n: &BigUint, e: &BigUint, message: &[u8], signature: &[u8]) -> bool {
    let k = n.to_bytes_be().len();
    let s = BigUint::from_bytes_be(signature);
    if signature.len() != k || &s >= n || k < 62 {
        return false;
    }
    let m = s.modpow(e, n).to_bytes_be();
    let mut em = vec![0u8; k - m.len()];
    em.extend_from_slice(&m);
    // 00 01 FF…FF 00, puis DigestInfo(SHA‑256) et l'empreinte.
    const SHA256_INFO: [u8; 19] = [
        0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
        0x05, 0x00, 0x04, 0x20,
    ];
    let digest = Sha256::digest(message);
    let mut expected = vec![0x00, 0x01];
    expected.resize(k - SHA256_INFO.len() - digest.len() - 1, 0xff);
    expected.push(0x00);
    expected.extend_from_slice(&SHA256_INFO);
    expected.extend_from_slice(&digest);
    em == expected
}

/// Le base64url sans remplissage des JWT.
fn base64url(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return Err("jeton illisible".into()),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une clé d'essai (pas celle d'Apple) et des jetons signés avec elle : le bon, un
    /// pour une autre app, un au courriel non vérifié, un au courriel masqué.
    fn vectors() -> Vec<&'static str> {
        include_str!("apple-essai.txt").lines().collect()
    }

    #[test]
    fn a_genuine_token_gives_its_verified_email() {
        let v = vectors();
        assert_eq!(
            verify_with(v[1], v[0], "app.luma.studio", 1500),
            Ok("max@exemple.fr".into())
        );
    }

    #[test]
    fn a_forged_expired_or_foreign_token_is_refused() {
        let v = vectors();
        let (keys, good) = (v[0], v[1]);
        // Expiré, ou daté du futur.
        assert!(verify_with(good, keys, "app.luma.studio", 2000).is_err());
        assert!(verify_with(good, keys, "app.luma.studio", 600).is_err());
        // Pour une autre app.
        assert!(verify_with(good, keys, "com.autre.app", 1500).is_err());
        assert!(verify_with(v[2], keys, "app.luma.studio", 1500).is_err());
        // Courriel non vérifié, ou masqué (le relais d'Apple n'est dans aucune liste).
        assert!(verify_with(v[3], keys, "app.luma.studio", 1500).is_err());
        assert!(
            verify_with(v[4], keys, "app.luma.studio", 1500)
                .unwrap_err()
                .contains("masquée")
        );
        // Le corps changé (un autre courriel) : la signature ne tient plus.
        let parts: Vec<&str> = good.split('.').collect();
        let mut claims: Value = serde_json::from_slice(&base64url(parts[1]).unwrap()).unwrap();
        claims["email"] = "eve@exemple.fr".into();
        let body = base64url_encode(claims.to_string().as_bytes());
        let forged = format!("{}.{body}.{}", parts[0], parts[2]);
        assert_eq!(
            verify_with(&forged, keys, "app.luma.studio", 1500),
            Err("jeton refusé (signature)".into())
        );
        // Une clé inconnue, un jeton tronqué, un autre algorithme.
        assert!(verify_with(good, r#"{"keys":[]}"#, "app.luma.studio", 1500).is_err());
        assert!(verify_with("abc.def", keys, "app.luma.studio", 1500).is_err());
        let none = format!(
            "{}.{}.",
            base64url_encode(br#"{"alg":"none","kid":"essai"}"#),
            parts[1]
        );
        assert!(verify_with(&none, keys, "app.luma.studio", 1500).is_err());
    }

    fn base64url_encode(bytes: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for i in 0..=chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            }
        }
        out
    }
}
