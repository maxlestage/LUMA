//! Les textes et les données du site, partagés par les pages (serveur) et les îlots
//! (serveur et navigateur). Les textes sont en français : `crate::i18n::t` les traduit.

use crate::i18n::{Lang, t};

pub const TITLE: &str = "LÜMA — La lumière, développée en Rust";
pub const DESCRIPTION: &str = "LÜMA, l'éditeur photo professionnel écrit en Rust : RAW, couleur 32 bits flottants sur le GPU, 100 films, vos vraies pellicules mesurées, masques. Mac, iPhone et iPad. 14 jours d'essai.";

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
        "19,99 €",
        "par mois",
        "Sans engagement, résiliable à tout moment.",
        "LUMA_CHECKOUT_MONTHLY",
    ),
    (
        "Annuel",
        "239,88 €",
        "par an",
        "Soit 19,99 € par mois, en un seul paiement.",
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
    "Sur Mac, avec toutes les mises à jour",
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

/// Les panneaux de la section Apple : (titre, texte). Dans un titre, le mot entre `*`
/// ressort en italique.
pub const APPLE: [(&str, &str); 4] = [
    (
        "Conçu pour *Apple Silicon*.",
        "Un seul GPU à dompter au départ : Metal, via wgpu. La mémoire unifiée laisse le RAW décodé passer au GPU sans copie.",
    ),
    (
        "La couleur, *comme elle est*.",
        "Display P3 partout, HDR (EDR) sur les écrans XDR. Le meilleur terrain pour valider un pipeline 32 bits flottants.",
    ),
    (
        "Le même moteur, *dans ta poche*.",
        "ProRAW sur iPhone, réglé du bout du pouce : aperçu réduit pendant le geste, pleine netteté au lâcher.",
    ),
    (
        "Les masques, *au crayon*.",
        "Sur iPad, l'Apple Pencil peint les masques à même la photo. Le cœur Rust est identique sur Mac, iPhone et iPad : même projet, même rendu.",
    ),
];

/// (plateforme, phase, détail, prioritaire)
pub const PLATFORMS: [(&str, &str, &str, bool); 6] = [
    (
        "macOS",
        "Phase 1",
        "Apple Silicon, Metal, EDR, téléchargement direct.",
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
    ("P2", "P2 · outils avancés"),
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
        name: "luma-quality",
        priority: "P2",
        glyph: "◉",
        text: "Dématriçage fin, débruitage avancé, déconvolution. Sans IA.",
    },
    Crate {
        name: "luma-privacy",
        priority: "P2",
        glyph: "◌",
        text: "Diagnostics sans contenu photographique, rien envoyé sans accord.",
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

/// Une mesure de la section Performance.
pub struct Perf {
    pub name: &'static str,
    pub unit: &'static str,
    /// Mesuré sur Mac : médiane de cinq vrais RAW (Sony, Canon, Nikon, Fujifilm, iPhone
    /// ProRAW), par `cargo run --release -p luma-gpu --example measure` dans la CI macOS
    /// (GPU Metal). Affiché avec `decimals` décimales.
    pub mac: f32,
    pub decimals: usize,
    /// Les objectifs, sur Mac et sur iPhone (pas encore mesuré).
    pub mac_target: u32,
    pub phone_target: u32,
    /// Une précision, avec sa valeur à la place de `{v}`.
    pub note: Option<(&'static str, f32)>,
}

/// Ce qui est déjà fait pour iPhone et iPad, avec le gain mesuré : (titre, détail).
pub const PHONE_GAINS: [(&str, &str); 5] = [
    (
        "Les curseurs suivent le doigt",
        "Pendant un geste, un aperçu plus petit, puis la pleine netteté au lâcher : 12 fois plus rapide (417 → 35 ms par image).",
    ),
    (
        "Tout le rendu sur le GPU",
        "Accentuation et Masquage compris, avec Metal : 15 fois plus rapide sur les grands écrans comme celui de l'iPad Pro.",
    ),
    (
        "Jamais fermée faute de mémoire",
        "LÜMA surveille la mémoire que le système lui laisse et libère ses caches sous 300 Mo, avant qu'il ne ferme l'app.",
    ),
    (
        "Photos en portrait plus vite ouvertes",
        "L'image est réduite avant d'être tournée : 5 fois moins de temps pour la tourner (91 → 18 ms pour un ProRAW de l'iPhone).",
    ),
    (
        "Vignettes instantanées",
        "L'aperçu intégré au RAW en 15 ms, puis les vignettes retouchées, relues en moins d'une milliseconde.",
    ),
];

pub const PERF: [Perf; 5] = [
    Perf {
        name: "Aperçu après réglage",
        unit: "ms",
        mac: 4.9,
        decimals: 1,
        mac_target: 100,
        phone_target: 150,
        note: None,
    },
    Perf {
        name: "Réglage sur écran Retina",
        unit: "ms",
        mac: 22.5,
        decimals: 1,
        mac_target: 100,
        phone_target: 150,
        note: Some((
            "Aperçu de 3072 px avec l'accentuation, tout sur le GPU. Sur le processeur : {v} ms.",
            341.0,
        )),
    },
    Perf {
        name: "Ouverture d'une photo",
        unit: "ms",
        mac: 97.0,
        decimals: 0,
        mac_target: 500,
        phone_target: 800,
        note: Some(("La première fois, depuis le RAW : {v} ms.", 177.0)),
    },
    Perf {
        name: "Vignette",
        unit: "ms",
        mac: 15.0,
        decimals: 0,
        mac_target: 150,
        phone_target: 250,
        note: Some((
            "L'aperçu intégré au RAW, aussitôt ; puis le vrai rendu : {v} ms.",
            154.0,
        )),
    },
    Perf {
        name: "Mémoire pendant la retouche",
        unit: "Mo",
        mac: 64.0,
        decimals: 0,
        mac_target: 500,
        phone_target: 300,
        note: Some(("Depuis le RAW, sans doublon : {v} Mo.", 163.0)),
    },
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
            "Masques par plages, débruitage avancé, sans IA",
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
            "App Store (iPhone et iPad), téléchargement direct (Mac)",
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

/// La valeur d'un réglage telle qu'affichée : `+0.15 IL` (`EV` en anglais et en
/// espagnol), `-55`.
pub fn format_control(lang: Lang, key: &str, v: f64) -> String {
    let sign = if v >= 0.0 { "+" } else { "" };
    if key == "exposure" {
        format!("{sign}{v:.2} {}", t(lang, "IL"))
    } else {
        format!("{sign}{}", (v * 100.0).round() as i64)
    }
}

/// Le pied de page : des colonnes de liens (titre, puis libellé et adresse ; une adresse
/// en `#` est une section de l'accueil, en `/` une page du site, sinon un lien externe).
pub const FOOTER: [(&str, &[(&str, &str)]); 4] = [
    (
        "Produit",
        &[
            ("Le moteur", "#pipeline"),
            ("Le labo", "#labo"),
            ("Mac, iPhone et iPad", "#apple"),
            ("Tarifs", "#tarifs"),
            ("Feuille de route", "#roadmap"),
        ],
    ),
    (
        "Ressources",
        &[
            ("Boîtiers pris en charge", "/docs/boitiers/"),
            ("Fonctions", "/docs/fonctions/"),
            ("Architecture", "/docs/architecture/"),
            ("Performances", "#performance"),
            ("Notes de version", "/nouveautes/"),
        ],
    ),
    (
        "Assistance",
        &[
            ("Abonnement et clé de licence", "/conditions/#abonnement"),
            ("Gérer mon abonnement", "/abonnement/"),
            ("Signaler un problème", "/mentions-legales/#contact"),
            ("Contact", "/mentions-legales/#contact"),
        ],
    ),
    (
        "Légal",
        &[
            ("Mentions légales", "/mentions-legales/"),
            ("Confidentialité", "/confidentialite/"),
            ("Conditions d'abonnement", "/conditions/"),
            ("Aucun cookie", "/confidentialite/#cookies"),
        ],
    ),
];

/// Une page légale : adresse, titre, chapeau, et ses sections (ancre, titre,
/// paragraphes). Un paragraphe entre `[[…]]` est à compléter par l'éditeur.
pub struct Legal {
    pub path: &'static str,
    pub title: &'static str,
    pub lede: &'static str,
    pub sections: &'static [(&'static str, &'static str, &'static [&'static str])],
}

pub const LEGAL: [Legal; 3] = [
    Legal {
        path: "/mentions-legales/",
        title: "Mentions légales",
        lede: "Qui publie LÜMA, qui l'héberge, et à qui écrire.",
        sections: &[
            (
                "editeur",
                "Éditeur",
                &[
                    "[[Nom ou raison sociale, forme juridique, capital, adresse du siège, numéro SIREN ou RCS, numéro de TVA intracommunautaire.]]",
                    "[[Directeur de la publication : prénom et nom.]]",
                ],
            ),
            (
                "contact",
                "Contact",
                &[
                    "[[Adresse électronique de contact.]]",
                    "Pour un problème dans l'app : écrivez à l'adresse de contact, avec la version de LÜMA et, si possible, le RAW concerné.",
                ],
            ),
            (
                "hebergement",
                "Hébergement",
                &[
                    "Le site est hébergé par Heroku : Salesforce, Inc., Salesforce Tower, 415 Mission Street, San Francisco, CA 94105, États‑Unis.",
                    "Les paiements sont traités par Stripe : Stripe Payments Europe, Ltd., 1 Grand Canal Street Lower, Dublin 2, Irlande.",
                ],
            ),
            (
                "marques",
                "Marques et noms de pellicules",
                &[
                    "LÜMA, son logo et son interface sont la propriété de leur éditeur.",
                    "Les noms de pellicules et de procédés cités dans les films de LÜMA (Kodak, Fujifilm, Ilford, Agfa, CineStill, Polaroid, Lomography…) sont des marques de leurs propriétaires. Ils disent l'inspiration d'un rendu, sans lien avec ces marques ni copie de leurs produits.",
                ],
            ),
        ],
    },
    Legal {
        path: "/confidentialite/",
        title: "Confidentialité",
        lede: "Vos photos restent chez vous. LÜMA ne vous suit pas.",
        sections: &[
            (
                "photos",
                "Vos photos",
                &[
                    "LÜMA développe vos photos sur votre appareil. Aucune photo, aucun réglage, aucune métadonnée n'est envoyé ailleurs : ni à l'éditeur, ni à un service tiers.",
                    "Les réglages sont écrits à côté de vos RAW, dans des fichiers XMP que vous gardez.",
                ],
            ),
            (
                "licence",
                "Abonnement",
                &[
                    "Votre clé de licence est vérifiée par l'app elle‑même, sans réseau. Une fois par jour, en ligne, l'app envoie votre clé au serveur de LÜMA pour en recevoir une à jour ; le serveur demande à Stripe si l'abonnement court. Rien d'autre n'est envoyé.",
                    "Lors d'un achat, Stripe traite votre nom, votre courriel et votre paiement, selon sa propre politique de confidentialité ; l'éditeur ne voit jamais vos coordonnées bancaires. La clé de licence contient le courriel de l'abonnement.",
                ],
            ),
            (
                "mesure",
                "Aucune mesure d'audience",
                &[
                    "Ni l'app ni le site ne contiennent de traceur, de publicité ou de mesure d'audience.",
                ],
            ),
            (
                "cookies",
                "Aucun cookie",
                &[
                    "Le site ne dépose aucun cookie. Installé comme une app (PWA), il garde une copie de ses pages dans votre navigateur, pour s'ouvrir hors ligne : vous pouvez l'effacer à tout moment dans les réglages du navigateur.",
                    "Le site charge ses polices depuis Google Fonts : votre navigateur contacte alors les serveurs de Google. Il est hébergé par Heroku, qui tient des journaux techniques (adresse IP) pour la sécurité du service.",
                ],
            ),
            (
                "droits",
                "Vos droits",
                &[
                    "Conformément au RGPD, vous pouvez demander l'accès, la rectification ou l'effacement des données liées à votre abonnement, à l'adresse de contact des mentions légales. Vous pouvez aussi saisir la CNIL.",
                ],
            ),
        ],
    },
    Legal {
        path: "/conditions/",
        title: "Conditions d'abonnement",
        lede: "Quatorze jours pour essayer, puis au mois ou à l'an. Sans surprise.",
        sections: &[
            (
                "essai",
                "Essai gratuit",
                &[
                    "LÜMA se télécharge librement. Au premier lancement commence un essai de 14 jours, complet, sans carte bancaire. À la fin de l'essai, l'app demande un abonnement pour continuer à développer vos photos ; vos fichiers et vos réglages restent à vous.",
                ],
            ),
            (
                "abonnement",
                "Abonnement et clé de licence",
                &[
                    "Deux formules : Mensuel, 19,99 € par mois, et Annuel, 239,88 € par an (TVA comprise). L'abonnement se renouvelle automatiquement à la fin de chaque période.",
                    "Après le paiement, votre clé de licence s'affiche sur le site (gardez l'adresse de la page : elle redonne la clé à tout moment). Saisissez‑la dans LÜMA (puce « Essai » en haut de l'app) ; elle vaut pour vos propres appareils, pour votre usage personnel ou professionnel.",
                    "La clé est valable jusqu'à la fin de la période payée, plus 14 jours. En ligne, l'app la renouvelle d'elle‑même chaque jour tant que l'abonnement court.",
                    "Sur Mac, LÜMA Premium s'achète uniquement sur ce site, payé par Stripe ; il n'est pas vendu sur le Mac App Store. Sur iPhone et iPad (iOS et iPadOS), il s'achète uniquement dans l'App Store, par un abonnement intégré distinct de l'abonnement Mac : il est facturé par Apple, aux prix affichés dans l'App Store, et se gère ou se résilie dans les réglages de votre compte Apple. Les articles Paiement et Résiliation ci‑dessous concernent l'abonnement Mac.",
                ],
            ),
            (
                "paiement",
                "Paiement",
                &[
                    "Les paiements sont encaissés par Stripe, par carte bancaire et les moyens proposés sur la page de paiement. La TVA de votre pays est appliquée ; la facture est envoyée par courriel à chaque paiement.",
                ],
            ),
            (
                "resiliation",
                "Résiliation et remboursement",
                &[
                    "Vous pouvez résilier à tout moment depuis le portail client (lien « Gérer mon abonnement ») : factures, moyen de paiement, résiliation. L'abonnement reste actif jusqu'à la fin de la période payée, puis LÜMA revient à l'écran Premium.",
                    "LÜMA étant un contenu numérique fourni dès l'activation, avec votre accord, le droit de rétractation ne s'applique plus une fois la clé activée (article L221‑28 du Code de la consommation). Pour toute demande de remboursement, écrivez à l'adresse de contact des mentions légales.",
                ],
            ),
            (
                "evolutions",
                "Mises à jour",
                &[
                    "Toutes les mises à jour de LÜMA sont comprises dans l'abonnement, sur Mac, iPhone et iPad. Une modification des prix est annoncée au moins 30 jours à l'avance et ne s'applique qu'au renouvellement suivant.",
                ],
            ),
            (
                "droit",
                "Droit applicable",
                &[
                    "Ces conditions sont soumises au droit français. En cas de litige, une solution amiable est recherchée d'abord ; à défaut, vous pouvez recourir gratuitement à un médiateur de la consommation.",
                ],
            ),
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_controls() {
        assert_eq!(format_control(Lang::Fr, "exposure", 0.15), "+0.15 IL");
        assert_eq!(format_control(Lang::En, "exposure", -0.2), "-0.20 EV");
        assert_eq!(format_control(Lang::Fr, "highlights", -0.55), "-55");
        assert_eq!(format_control(Lang::Es, "sat", 0.0), "+0");
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
