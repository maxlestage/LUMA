//! Les textes et les données du site, partagés par les pages (serveur) et les îlots
//! (serveur et navigateur).

pub const TITLE: &str = "LÜMA — La lumière, développée en Rust";
pub const DESCRIPTION: &str = "LÜMA, l'éditeur photo professionnel écrit en Rust : RAW, couleur 32 bits flottants sur le GPU, 100 films, vos vraies pellicules mesurées, masques. Mac, iPhone et iPad. 14 jours d'essai.";
pub const REPO: &str = "https://github.com/maxlestage/LUMA";

pub const NAV: [(&str, &str); 6] = [
    ("#pipeline", "Moteur"),
    ("#labo", "Labo"),
    ("#apple", "Apple"),
    ("#architecture", "Architecture"),
    ("#roadmap", "Feuille de route"),
    ("#tarifs", "Tarifs"),
];

/// LÜMA Premium : les formules (nom, prix, période, note, variable du lien de
/// paiement). Les mêmes que dans l'app (`luma-licence`).
pub const PLANS: [(&str, &str, &str, &str, &str); 2] = [
    (
        "Mensuel",
        "9,99 €",
        "par mois",
        "Sans engagement, résiliable à tout moment.",
        "LUMA_CHECKOUT_MONTHLY",
    ),
    (
        "Annuel",
        "79,99 €",
        "par an",
        "Deux mois offerts : 6,67 € par mois.",
        "LUMA_CHECKOUT_ANNUAL",
    ),
];

/// Ce que LÜMA Premium comprend (comme dans l'app).
pub const PREMIUM: [&str; 6] = [
    "Le développement RAW complet, sur le GPU",
    "100 films, vos préréglages, et vos pellicules mesurées",
    "Masques, films empilés, recadrage et géométrie",
    "Empilements, ciel étoilé, panoramas en DNG",
    "Export JPEG, PNG, TIFF 16 bits, en P3 et ProPhoto",
    "Mac, iPhone et iPad, et toutes les mises à jour",
];

pub const MARQUEE: [&str; 9] = [
    "RAW 14 & 16 bits",
    "Pipeline f32",
    "Display P3",
    "HDR / EDR",
    "Metal",
    "Apple Pencil",
    "100 films · LUT 3D",
    "Non destructif",
    "Zéro cloud obligatoire",
];

/// Le manifeste, mot à mot ; les mots marqués d'un `*` ressortent en couleur.
pub const MANIFESTO: &str = "Être écrit en Rust ne rend pas un logiciel meilleur. Ce qui compte, c'est la qualité du moteur *RAW, la rigueur de la *couleur et la *vitesse ressentie sous les doigts. LÜMA veut gagner sur ces trois terrains, et le *prouver.";

pub struct Stat {
    pub prefix: &'static str,
    pub value: u32,
    pub unit: &'static str,
    pub text: &'static str,
}

pub const STATS: [Stat; 4] = [
    Stat {
        prefix: "",
        value: 32,
        unit: "bits",
        text: "Pipeline en virgule flottante, du capteur à l'export.",
    },
    Stat {
        prefix: "<",
        value: 100,
        unit: "ms",
        text: "Objectif d'aperçu après un réglage (P95, Mac).",
    },
    Stat {
        prefix: "",
        value: 120,
        unit: "Hz",
        text: "Interface pensée pour ProMotion sur Mac, iPhone et iPad.",
    },
    Stat {
        prefix: "",
        value: 0,
        unit: "compte",
        text: "Aucun compte ni cloud obligatoire. Ta photothèque reste chez toi.",
    },
];

/// Les étapes du pipeline : (nom, détail, étiquette de la couche).
pub const PIPELINE: [(&str, &str, &str); 5] = [
    ("Original", "RAW, JPEG ou HEIF. Jamais modifié.", "RAW"),
    (
        "Décodage",
        "Bayer, X‑Trans, ProRAW, dématriçage, profil caméra.",
        "Décodage",
    ),
    (
        "Couleur",
        "Espace linéaire f32, XYZ, Display P3, Rec. 2020.",
        "Couleur",
    ),
    (
        "Masques",
        "Luminance, couleur, pinceau, sujet, ciel.",
        "Masques",
    ),
    (
        "Rendu",
        "Aperçu en tuiles sur GPU, export pleine précision.",
        "Rendu",
    ),
];

/// Les panneaux de la section Apple : (début du titre, mot en italique, texte).
pub const APPLE: [(&str, &str, &str); 3] = [
    (
        "Conçu pour ",
        "Apple Silicon",
        "Un seul GPU à dompter au départ : Metal, via wgpu. La mémoire unifiée laisse le RAW décodé passer au GPU sans copie.",
    ),
    (
        "La couleur, ",
        "comme elle est",
        "Display P3 partout, HDR (EDR) sur les écrans XDR. Le meilleur terrain pour valider un pipeline 32 bits flottants.",
    ),
    (
        "Le même moteur, ",
        "dans ta poche",
        "ProRAW sur iPhone, Apple Pencil sur iPad pour les masques. Le cœur Rust est identique sur Mac, iPhone et iPad : même projet, même rendu.",
    ),
];

/// (plateforme, phase, détail, prioritaire)
pub const PLATFORMS: [(&str, &str, &str, bool); 6] = [
    (
        "macOS",
        "Phase 1",
        "Apple Silicon, Metal, EDR, Mac App Store.",
        true,
    ),
    (
        "iOS",
        "Phase 1",
        "ProRAW, PhotoKit, Fichiers, TestFlight.",
        true,
    ),
    (
        "iPadOS",
        "Phase 3",
        "Apple Pencil, gestes, Stage Manager.",
        true,
    ),
    ("Windows", "Phase 6", "DX12 / Vulkan via wgpu.", false),
    ("Linux", "Phase 6", "Vulkan, Wayland et X11.", false),
    ("Android", "Phase 6", "Vulkan, NDK, stylet.", false),
];

pub struct Crate {
    pub name: &'static str,
    pub priority: &'static str,
    pub glyph: &'static str,
    pub text: &'static str,
}

pub const PRIORITIES: [(&str, &str); 5] = [
    ("all", "Tout"),
    ("P0", "P0 · fondations"),
    ("P1", "P1 · éditeur"),
    ("P2", "P2 · IA"),
    ("P3", "P3 · expansion"),
];

pub const CRATES: [Crate; 18] = [
    Crate {
        name: "luma-core",
        priority: "P0",
        glyph: "⌘",
        text: "Commandes, annuler/rétablir, historique, tâches asynchrones, erreurs.",
    },
    Crate {
        name: "luma-project",
        priority: "P0",
        glyph: "◫",
        text: "Format de projet versionné et migrable, sauvegarde atomique.",
    },
    Crate {
        name: "luma-raw",
        priority: "P0",
        glyph: "▦",
        text: "DNG, Bayer, X‑Trans, ProRAW, dématriçage, EXIF, profils caméra.",
    },
    Crate {
        name: "luma-color",
        priority: "P0",
        glyph: "◐",
        text: "ICC, XYZ, sRGB, Display P3, Rec. 2020, EDR/HDR, flottants 32 bits.",
    },
    Crate {
        name: "luma-image",
        priority: "P0",
        glyph: "☀",
        text: "Exposition, courbes, HSL, netteté, grain, vignettage, géométrie.",
    },
    Crate {
        name: "luma-render",
        priority: "P0",
        glyph: "◇",
        text: "Graphe de traitement, tuiles, cache multi‑résolution, annulation.",
    },
    Crate {
        name: "luma-gpu",
        priority: "P0",
        glyph: "▲",
        text: "wgpu sur Metal d'abord, shaders WGSL, repli sur le CPU.",
    },
    Crate {
        name: "luma-ui",
        priority: "P0",
        glyph: "◰",
        text: "Interface adaptative : pointeur, tactile, Apple Pencil, 120 Hz.",
    },
    Crate {
        name: "luma-platform",
        priority: "P0",
        glyph: "◈",
        text: "macOS et iOS d'abord : fichiers, PhotoKit, caméra, EDR, permissions.",
    },
    Crate {
        name: "luma-mask",
        priority: "P1",
        glyph: "◎",
        text: "Luminance, couleur, pinceau, dégradés, unions et intersections.",
    },
    Crate {
        name: "luma-library",
        priority: "P1",
        glyph: "▤",
        text: "Catalogue local, collections, notes, doublons, recherche.",
    },
    Crate {
        name: "luma-export",
        priority: "P1",
        glyph: "⇪",
        text: "JPEG, PNG, TIFF, DNG, lots, préréglages, métadonnées.",
    },
    Crate {
        name: "luma-presets",
        priority: "P1",
        glyph: "✦",
        text: "Styles et profils de rendu partageables.",
    },
    Crate {
        name: "luma-ai",
        priority: "P2",
        glyph: "◉",
        text: "Sujet, ciel, débruitage, super‑résolution. Local, modèles versionnés.",
    },
    Crate {
        name: "luma-privacy",
        priority: "P2",
        glyph: "◌",
        text: "Consentement IA, diagnostics sans contenu photographique.",
    },
    Crate {
        name: "luma-sync",
        priority: "P3",
        glyph: "⇄",
        text: "Synchronisation chiffrée et facultative entre appareils.",
    },
    Crate {
        name: "luma-plugins",
        priority: "P3",
        glyph: "⬡",
        text: "Extensions isolées, permissions, API stable.",
    },
    Crate {
        name: "luma-video",
        priority: "P3",
        glyph: "▶",
        text: "Timeline, étalonnage, rendu. Le module le plus risqué.",
    },
];

/// (mesure, unité, Mac, iPhone) : des objectifs, pas des mesures.
pub const PERF: [(&str, &str, u32, u32); 4] = [
    ("Commande simple", "ms", 50, 80),
    ("Aperçu après réglage", "ms", 100, 150),
    ("Vignette", "ms", 150, 250),
    ("Mémoire au repos", "Mo", 500, 300),
];

pub struct Phase {
    pub when: &'static str,
    pub name: &'static str,
    pub items: &'static [&'static str],
    pub goal: &'static str,
    pub later: bool,
}

pub const ROADMAP: [Phase; 6] = [
    Phase {
        when: "M0 – M3",
        name: "Faisabilité",
        items: &[
            "App macOS et iOS minimales en Rust",
            "Rendu wgpu sur Metal",
            "RAW : Sony, Canon, Nikon, Fujifilm, ProRAW",
            "Signature et TestFlight automatisés",
        ],
        goal: "Un RAW s'ouvre, se règle et s'exporte sur Mac et iPhone.",
        later: false,
    },
    Phase {
        when: "M4 – M6",
        name: "Moteur",
        items: &[
            "Pipeline f32, Display P3",
            "HDR / EDR sur écrans XDR",
            "Mémoire unifiée sans copie",
            "Graphe de retouche, projet versionné",
        ],
        goal: "Même rendu Mac et iPhone, à tolérance ΔE documentée.",
        later: false,
    },
    Phase {
        when: "M7 – M12",
        name: "Alpha",
        items: &[
            "Photothèque, retouche, export sur Mac",
            "Import PhotoKit et Fichiers",
            "Masques à l'Apple Pencil",
            "TestFlight public",
        ],
        goal: "Un flux photo complet sans autre éditeur.",
        later: false,
    },
    Phase {
        when: "M13 – M18",
        name: "Bêta",
        items: &[
            "IA locale : sujet, ciel, débruitage",
            "Profils boîtiers et objectifs",
            "Recherche, doublons",
            "Récupération après plantage",
        ],
        goal: "Outils avancés validés en qualité et en vitesse.",
        later: false,
    },
    Phase {
        when: "M19 – M24",
        name: "1.0",
        items: &[
            "Mac App Store et App Store",
            "Accessibilité, migration",
            "Matrice de compatibilité publique",
        ],
        goal: "Version photo professionnelle stable.",
        later: false,
    },
    Phase {
        when: "M25 – M36",
        name: "Expansion",
        items: &[
            "Vidéo : timeline, étalonnage",
            "Tethering, impression",
            "Windows, Linux, Android",
        ],
        goal: "Le même moteur, partout.",
        later: true,
    },
];

/// `deny.toml`, ligne par ligne : (classe de couleur, texte). Dans le texte, `"…"` est
/// une chaîne et `$` une invite.
pub const DENY: [(&str, &str); 18] = [
    ("c", "# deny.toml — vérifié à chaque build"),
    ("k", "[bans]"),
    ("", "deny = ["),
    ("", "  { name = \"openssl-sys\" },"),
    ("", "  { name = \"libraw-sys\" },"),
    ("", "  { name = \"ffmpeg-sys-next\" },"),
    ("", "]"),
    ("", ""),
    ("k", "[sources]"),
    ("", "unknown-registry = \"deny\""),
    ("", "unknown-git      = \"deny\""),
    ("", ""),
    ("k", "[licenses]"),
    (
        "",
        "allow = [\"MIT\", \"Apache-2.0\", \"BSD-3-Clause\", \"ISC\", \"Zlib\"]",
    ),
    ("", ""),
    ("c", "# cibles prioritaires"),
    ("", "$ cargo deny --target aarch64-apple-darwin check"),
    ("", "$ cargo deny --target aarch64-apple-ios check"),
];

pub const RULES: [(&str, &str); 4] = [
    ("Épinglé", "Dépendances transitives auditées et figées."),
    (
        "Sans FFI tiers",
        "Pas de LibRaw, d'OpenSSL ni de FFmpeg cachés.",
    ),
    ("unsafe encadré", "Limité, documenté, testé."),
    (
        "Reproductible",
        "Corpus RAW de référence, rendus dorés, tolérances ΔE.",
    ),
];

// ----- Labo ---------------------------------------------------------------------------

/// Un réglage du labo : (clé, libellé, min, max, classe de la piste).
pub const CONTROLS: [(&str, &str, f64, f64, &str); 8] = [
    ("exposure", "Exposition", -2.0, 2.0, ""),
    ("contrast", "Contraste", -1.0, 1.0, ""),
    ("highlights", "Hautes lumières", -1.0, 1.0, ""),
    ("shadows", "Ombres", -1.0, 1.0, ""),
    ("temp", "Température", -1.0, 1.0, "warm"),
    ("tint", "Teinte", -1.0, 1.0, "tint"),
    ("sat", "Saturation", -1.0, 1.0, ""),
    ("vignette", "Vignettage", 0.0, 1.0, ""),
];

/// Les préréglages, dans l'ordre de [`CONTROLS`].
pub const PRESETS: [(&str, &str, [f64; 8]); 5] = [
    (
        "natural",
        "Naturel",
        [0.15, 0.12, -0.55, 0.4, 0.08, 0.0, 0.12, 0.2],
    ),
    (
        "golden",
        "Golden hour",
        [0.3, 0.18, -0.7, 0.5, 0.6, 0.15, 0.3, 0.35],
    ),
    (
        "cine",
        "Ciné",
        [-0.2, 0.4, -0.4, 0.2, -0.35, 0.3, -0.15, 0.6],
    ),
    ("mono", "N&B", [0.1, 0.45, -0.5, 0.3, 0.0, 0.0, -1.0, 0.45]),
    ("reset", "Original", [0.0; 8]),
];

/// La valeur d'un réglage telle qu'affichée : `+0.15 IL`, `-55`.
pub fn format_control(key: &str, v: f64) -> String {
    let sign = if v >= 0.0 { "+" } else { "" };
    if key == "exposure" {
        format!("{sign}{v:.2} IL")
    } else {
        format!("{sign}{}", (v * 100.0).round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_controls() {
        assert_eq!(format_control("exposure", 0.15), "+0.15 IL");
        assert_eq!(format_control("highlights", -0.55), "-55");
        assert_eq!(format_control("sat", 0.0), "+0");
    }

    #[test]
    fn presets_cover_every_control() {
        for (_, _, values) in PRESETS {
            for ((_, _, min, max, _), v) in CONTROLS.iter().zip(values) {
                assert!((*min..=*max).contains(&v));
            }
        }
    }
}
