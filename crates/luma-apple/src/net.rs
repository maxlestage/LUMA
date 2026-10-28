//! Le téléchargement d'un fichier (les tuiles de la carte) et l'envoi d'un formulaire
//! (le serveur des licences, sur iPhone et iPad), par le système : sur Mac, iPhone et
//! iPad, Foundation sait le HTTPS sans qu'aucune bibliothèque de chiffrement n'entre
//! dans LÜMA. Ailleurs, rien ne se télécharge.

/// Le téléchargement est‑il possible ici ?
pub const AVAILABLE: bool = cfg!(any(target_os = "macos", target_os = "ios"));

/// Le contenu de cette adresse, ou `None` (pas de réseau, adresse refusée). Bloque
/// jusqu'à la réponse : à appeler hors du fil de l'interface.
pub fn fetch(url: &str) -> Option<Vec<u8>> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        use objc2_foundation::{NSData, NSString, NSURL};
        let url = NSURL::URLWithString(&NSString::from_str(url))?;
        let data = NSData::dataWithContentsOfURL(&url)?;
        Some(data.to_vec())
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = url;
        None
    }
}

/// Envoie le formulaire `body` (`application/x-www-form-urlencoded`) à `url` en POST,
/// et rend la réponse, quel que soit son code HTTP (le serveur des licences répond en
/// JSON même pour un refus) ; l'erreur du système sans réponse (pas de réseau). Bloque
/// jusqu'à la réponse (20 secondes au plus) : à appeler hors du fil de l'interface.
pub fn post(url: &str, body: &str, user_agent: &str) -> Result<Vec<u8>, String> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        use objc2_foundation::{NSData, NSMutableURLRequest, NSString, NSURL, NSURLConnection};
        let url = NSURL::URLWithString(&NSString::from_str(url)).ok_or("adresse invalide")?;
        let request = NSMutableURLRequest::requestWithURL(&url);
        request.setHTTPMethod(&NSString::from_str("POST"));
        request.setTimeoutInterval(20.0);
        for (field, value) in [
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Accept", "application/json"),
            ("User-Agent", user_agent),
        ] {
            request.setValue_forHTTPHeaderField(Some(&NSString::from_str(value)), &NSString::from_str(field));
        }
        request.setHTTPBody(Some(&NSData::with_bytes(body.as_bytes())));
        // La version synchrone, dépréciée au profit de NSURLSession mais toujours là :
        // l'appelant est déjà sur un fil à part.
        #[allow(deprecated)]
        let data = NSURLConnection::sendSynchronousRequest_returningResponse_error(&request, None)
            .map_err(|e| e.localizedDescription().to_string())?;
        Ok(data.to_vec())
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    {
        let _ = (url, body, user_agent);
        Err("pas de réseau ici".into())
    }
}
