//! Le téléchargement d'un fichier (les tuiles de la carte), par le système : sur Mac,
//! iPhone et iPad, Foundation sait le HTTPS sans qu'aucune bibliothèque de
//! chiffrement n'entre dans LÜMA. Ailleurs, rien ne se télécharge.

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
