//! La matrice des boîtiers : tous ceux que le décodeur connaît (la base de rawler), avec
//! leurs alias, et si leur couleur est étalonnée (matrice D65). `luma cameras --markdown`
//! l'écrit ; `docs/boitiers.md` en est la copie publiée, que la CI vérifie à chaque
//! version.

/// Un boîtier connu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camera {
    pub make: String,
    pub model: String,
    /// Les autres noms du même boîtier (selon les marchés).
    pub aliases: Vec<String>,
    /// Une matrice couleur D65 est connue : la couleur est étalonnée.
    pub color: bool,
}

/// Les boîtiers connus, triés par marque puis par modèle (sans doublons).
pub fn all() -> Vec<Camera> {
    let Ok(toml) = rawler::decoders::CAMERAS_TOML.parse::<toml::Table>() else {
        return Vec::new();
    };
    let s = |t: &toml::Table, k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    let mut out: Vec<Camera> = toml
        .get("cameras")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let t = c.as_table()?;
            let make = s(t, "clean_make").or_else(|| s(t, "make"))?;
            let model = s(t, "clean_model").or_else(|| s(t, "model"))?;
            let aliases = t
                .get("model_aliases")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|a| a.as_array()?.get(1)?.as_str().map(str::to_owned))
                .filter(|a| *a != model)
                .collect();
            let color = t
                .get("color_matrix")
                .and_then(|m| m.as_table())
                .is_some_and(|m| m.contains_key("D65"));
            Some(Camera {
                make,
                model,
                aliases,
                color,
            })
        })
        .collect();
    out.sort_by(|a, b| (a.make.to_lowercase(), &a.model).cmp(&(b.make.to_lowercase(), &b.model)));
    out.dedup_by(|a, b| a.make == b.make && a.model == b.model);
    out
}

/// La matrice en Markdown : par marque, chaque boîtier, sa couleur, et s'il est vérifié
/// sur le corpus de RAW réels (`tested` : marque et modèle).
pub fn markdown(tested: &[(&str, &str)]) -> String {
    let cams = all();
    let is_tested = |c: &Camera| {
        tested.iter().any(|(mk, md)| {
            mk.eq_ignore_ascii_case(&c.make)
                && (md.eq_ignore_ascii_case(&c.model) || c.aliases.iter().any(|a| a.eq_ignore_ascii_case(md)))
        })
    };
    let mut makes: Vec<&str> = cams.iter().map(|c| c.make.as_str()).collect();
    makes.dedup();
    let mut out = String::from(
        "# Boîtiers pris en charge\n\n\
         Généré par `luma cameras --markdown` depuis la base du décodeur ; la CI vérifie que ce\n\
         fichier est à jour. **Couleur** : matrice D65 connue (couleur étalonnée).\n\
         **Corpus** : un vrai RAW de ce boîtier est développé à chaque test\n\
         (`scripts/fetch-corpus.sh`). Les formats DNG (dont iPhone ProRAW) sont lus pour tous\n\
         les appareils, même absents de la liste.\n\n",
    );
    let colored = cams.iter().filter(|c| c.color).count();
    out += &format!(
        "**{} boîtiers** de {} marques ; {} avec couleur étalonnée ; {} vérifiés sur le corpus.\n\n",
        cams.len(),
        makes.len(),
        colored,
        cams.iter().filter(|c| is_tested(c)).count()
    );
    out += "| Marque | Boîtiers | Couleur étalonnée |\n|---|---|---|\n";
    for m in &makes {
        let of: Vec<&Camera> = cams.iter().filter(|c| c.make == *m).collect();
        out += &format!("| {m} | {} | {} |\n", of.len(), of.iter().filter(|c| c.color).count());
    }
    for m in &makes {
        out += &format!("\n## {m}\n\n| Boîtier | Couleur | Corpus |\n|---|---|---|\n");
        for c in cams.iter().filter(|c| c.make == *m) {
            let name = if c.aliases.is_empty() {
                c.model.clone()
            } else {
                format!("{} ({})", c.model, c.aliases.join(", "))
            };
            out += &format!(
                "| {} | {} | {} |\n",
                name.replace('|', "/"),
                if c.color { "✓" } else { "—" },
                if is_tested(c) { "✓" } else { "" }
            );
        }
    }
    out
}

/// Les boîtiers du corpus de RAW réels (`scripts/fetch-corpus.sh`).
pub const CORPUS: [(&str, &str); 5] = [
    ("Sony", "ILCE-7M3"),
    ("Canon", "EOS R5"),
    ("Nikon", "Z 6"),
    ("Fujifilm", "X-T3"),
    ("Apple", "iPhone 12 Pro"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hundreds_of_cameras_are_known_with_their_color() {
        let cams = all();
        assert!(cams.len() > 500, "{}", cams.len());
        assert!(cams.iter().filter(|c| c.color).count() * 10 > cams.len() * 9);
        for (mk, md) in CORPUS.iter().take(4) {
            assert!(
                cams.iter()
                    .any(|c| c.make.eq_ignore_ascii_case(mk) && c.model.eq_ignore_ascii_case(md)),
                "{mk} {md}"
            );
        }
        let md = markdown(&CORPUS);
        assert!(
            md.contains("## Sony") && md.contains("| ILCE-7M3 | ✓ | ✓ |"),
            "{}",
            &md[..md.len().min(800)]
        );
    }
}
