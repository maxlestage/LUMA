//! Les textes et les données du site, partagés par les pages (serveur) et les îlots
//! (serveur et navigateur). Les textes sont en français : `crate::i18n::t` les traduit.

use crate::i18n::{Lang, t};

pub const TITLE: &str = "LÜMA — L'éditeur photo qui développe la lumière";
pub const DESCRIPTION: &str = "LÜMA, l'éditeur photo professionnel : vos RAW développés avec précision, 275 films, vos vraies pellicules, des masques, une photothèque rapide. Sur Mac, iPhone, iPad et Linux. 14 jours d'essai.";

pub const NAV: [(&str, &str); 6] = [
    ("#etapes", "Fonctions"),
    ("#films", "Films"),
    ("#labo", "Essayer"),
    ("#apple", "Apple"),
    ("#questions", "Questions"),
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
    "275 films, vos préréglages, et vos pellicules mesurées",
    "Masques, films empilés, recadrage et géométrie",
    "Empilements, HDR, mise au point, ciel étoilé, panoramas en DNG",
    "Export JPEG, PNG, TIFF 16 bits, en P3 et ProPhoto",
    "Sur Mac et Linux, avec toutes les mises à jour",
];

pub const MARQUEE: [&str; 9] = [
    "735 boîtiers",
    "iPhone ProRAW",
    "275 films",
    "Vos pellicules",
    "Masques",
    "Photothèque",
    "Filigrane",
    "Originaux intacts",
    "Sans compte",
];

/// Le manifeste, mot à mot ; les mots marqués d'un `*` ressortent en couleur.
pub const MANIFESTO: &str = "Une belle photo commence à la prise de vue. LÜMA s'occupe du reste : un *RAW développé avec soin, des *couleurs justes, des réglages qui suivent le doigt *sans attendre, et des originaux qui ne sont *jamais modifiés.";

pub struct Stat {
    pub prefix: &'static str,
    pub value: u32,
    pub unit: &'static str,
    pub text: &'static str,
}

pub const STATS: [Stat; 4] = [
    Stat {
        prefix: "",
        value: 735,
        unit: "boîtiers",
        text: "Reconnus, de 20 marques, et tous les DNG, iPhone ProRAW compris.",
    },
    Stat {
        prefix: "",
        value: 275,
        unit: "films",
        text: "Pellicules couleur, noir et blanc, cinéma et instantanés, prêtes à poser.",
    },
    Stat {
        prefix: "",
        value: 14,
        unit: "jours",
        text: "D'essai complet, sans carte bancaire.",
    },
    Stat {
        prefix: "",
        value: 0,
        unit: "compte",
        text: "Aucun compte ni cloud obligatoire. Votre photothèque reste chez vous.",
    },
];

/// Les étapes du pipeline : (nom, détail, étiquette de la couche).
pub const PIPELINE: [(&str, &str, &str); 5] = [
    (
        "Original",
        "Votre RAW, JPEG ou HEIF, jamais modifié.",
        "Original",
    ),
    (
        "Lumière",
        "Exposition, contraste, hautes lumières, ombres, courbes.",
        "Lumière",
    ),
    (
        "Couleur",
        "Balance des blancs, teintes, saturation, profils.",
        "Couleur",
    ),
    (
        "Masques",
        "Dégradés, pinceau, plages de tons et de couleurs.",
        "Masques",
    ),
    (
        "Export",
        "JPEG, PNG ou TIFF 16 bits, en pleine qualité.",
        "Export",
    ),
];

/// Les quatre étapes du travail : (numéro, titre, texte).
pub const STEPS: [(&str, &str, &str); 4] = [
    (
        "01",
        "Ranger",
        "Chaque carte importée, chaque dossier ouvert entre dans la photothèque, sauvegardé sur un second disque si vous voulez. Notes, drapeaux, mots‑clés, collections, recherche, carte et renommage par lot : vos photos se retrouvent en un instant.",
    ),
    (
        "02",
        "Développer",
        "Lumière, couleur, courbes, détails (bruit par ondelettes, netteté par déconvolution), hautes lumières reconstruites, corrections d'objectif : chaque réglage s'affiche aussitôt, mesuré par la forme d'onde et le vectorscope, et l'original reste intact.",
    ),
    (
        "03",
        "Affiner",
        "Des masques pour retoucher le ciel, un visage ou un détail, et un film à poser où vous voulez dans l'image.",
    ),
    (
        "04",
        "Partager",
        "JPEG, PNG ou TIFF 16 bits, à la bonne taille, avec votre filigrane et vos informations d'auteur. Tirages au profil de l'imprimante, planches contact, livre photo, diaporama et galerie web.",
    ),
];

/// Quelques films, pour la section Films : (nom, famille, filtre CSS de l'aperçu).
pub const FILMS: [(&str, &str, &str); 12] = [
    (
        "Portra 400",
        "Couleur",
        "sepia(.18) saturate(1.05) brightness(1.06) contrast(.95)",
    ),
    ("Velvia 50", "Diapositive", "saturate(1.7) contrast(1.18)"),
    (
        "Kodachrome 64",
        "Diapositive",
        "sepia(.22) saturate(1.3) contrast(1.2)",
    ),
    (
        "CineStill 800T",
        "Cinéma",
        "hue-rotate(-14deg) saturate(1.25) brightness(.95) contrast(1.08)",
    ),
    ("Tri-X 400", "Noir et blanc", "grayscale(1) contrast(1.35)"),
    (
        "Polaroid SX-70",
        "Instantané",
        "sepia(.4) saturate(.8) brightness(1.1) contrast(.85)",
    ),
    ("Ektar 100", "Couleur", "saturate(1.45) contrast(1.1)"),
    (
        "Superia 400",
        "Couleur",
        "hue-rotate(10deg) saturate(1.15) contrast(1.04)",
    ),
    (
        "Lomochrome Purple",
        "Couleur",
        "hue-rotate(-70deg) saturate(1.2) contrast(1.05)",
    ),
    (
        "Aerochrome",
        "Couleur",
        "hue-rotate(160deg) saturate(1.5) contrast(1.1)",
    ),
    (
        "Néon",
        "Cinéma",
        "hue-rotate(-30deg) saturate(1.6) contrast(1.25) brightness(.9)",
    ),
    (
        "Pastel",
        "Créatif",
        "saturate(.75) brightness(1.15) contrast(.8)",
    ),
];

/// Vos photos restent chez vous : (titre, texte).
pub const PRIVACY: [(&str, &str); 4] = [
    (
        "Sans compte",
        "LÜMA s'ouvre et travaille sans inscription ni connexion.",
    ),
    (
        "Sans cloud obligatoire",
        "Vos photos et vos réglages restent sur votre disque, dans des fichiers que vous gardez.",
    ),
    (
        "Originaux intacts",
        "Vos réglages sont rangés à côté de chaque photo, dans un petit fichier que d'autres logiciels savent lire.",
    ),
    (
        "Position retirée",
        "À l'export, la position GPS est retirée par défaut : vous choisissez ce que vous partagez.",
    ),
];

/// Les questions fréquentes : (question, réponse).
pub const FAQ: [(&str, &str); 8] = [
    (
        "Quels appareils photo sont pris en charge ?",
        "735 boîtiers de 20 marques, dont Canon, Nikon, Sony, Fujifilm, Panasonic, Olympus et Leica, tous les fichiers DNG et l'iPhone ProRAW, ainsi que les JPEG, HEIC, PNG et TIFF. La liste complète est dans « Boîtiers pris en charge », en bas de page.",
    ),
    (
        "Mes photos sont‑elles modifiées ?",
        "Jamais. Vos réglages sont rangés dans un petit fichier à côté de la photo ; l'original reste tel qu'il est sorti de l'appareil.",
    ),
    (
        "Puis‑je reprendre mes réglages d'un autre logiciel ?",
        "LÜMA lit et écrit les fichiers XMP rangés à côté des photos, le format de réglages le plus répandu, et importe vos préréglages au format XMP.",
    ),
    (
        "Faut‑il un compte ou une connexion ?",
        "Non. LÜMA fonctionne sans compte. Sur Mac et Linux, une fois par jour s'il y a du réseau, l'app vérifie simplement que l'abonnement court ; sans réseau, elle continue.",
    ),
    (
        "Que se passe‑t‑il après l'essai ?",
        "Tout est ouvert pendant l'essai : 14 jours sur Mac et Linux, un mois sur iPhone et iPad. Ensuite, LÜMA Premium permet de continuer à développer (un abonnement sur Mac et Linux, un achat unique sur iPhone et iPad) ; vos fichiers et vos réglages restent à vous dans tous les cas.",
    ),
    (
        "Où acheter LÜMA ?",
        "Sur Mac et Linux, uniquement sur ce site, par abonnement (le même vaut pour les deux). Sur iPhone et iPad, uniquement dans l'App Store : 89,99 € une fois pour toutes, après un mois d'essai.",
    ),
    (
        "Où vont mes photos exportées sur iPhone ?",
        "Dans la Pellicule de l'app Photos, telles quelles, avec leurs informations de prise de vue et leur profil de couleur ; une copie reste aussi dans l'app Fichiers.",
    ),
    (
        "Et Linux, Windows ou Android ?",
        "Linux est déjà là : une archive qui s'installe sans mot de passe, ou un paquet .deb pour Debian et Ubuntu, avec le même abonnement que sur Mac. Windows et Android viendront ensuite, avec le même moteur : une photo aura le même rendu partout.",
    ),
];

/// Les panneaux de la section Apple : (titre, texte). Dans un titre, le mot entre `*`
/// ressort en italique.
pub const APPLE: [(&str, &str); 4] = [
    (
        "Conçu pour *Apple Silicon*.",
        "Les puces Apple développent vos RAW en direct : chaque réglage s'affiche aussitôt, même sur les gros fichiers.",
    ),
    (
        "La couleur, *comme elle est*.",
        "Des couleurs fidèles sur les écrans des Mac, des iPhone et des iPad, et des hautes lumières éclatantes sur les écrans XDR.",
    ),
    (
        "Le même moteur, *dans ta poche*.",
        "Vos ProRAW sur iPhone, réglés du bout du pouce : l'image suit le geste, retrouve toute sa netteté dès que vous lâchez, et l'export va droit dans la Pellicule.",
    ),
    (
        "Les masques, *au crayon*.",
        "Sur iPad, l'Apple Pencil peint les masques à même la photo. Et d'un appareil à l'autre, une photo garde le même rendu.",
    ),
];

/// (plateforme, phase, détail, prioritaire)
pub const PLATFORMS: [(&str, &str, &str, bool); 6] = [
    (
        "macOS",
        "D'abord",
        "Mac à puce Apple, écrans XDR, téléchargement direct.",
        true,
    ),
    ("iOS", "D'abord", "ProRAW, Photos, Fichiers.", true),
    (
        "iPadOS",
        "Ensuite",
        "Apple Pencil, gestes, Stage Manager.",
        true,
    ),
    (
        "Linux",
        "Disponible",
        "Archive ou paquet .deb, le même rendu que sur Mac.",
        true,
    ),
    ("Windows", "Plus tard", "Le même rendu que sur Mac.", false),
    (
        "Android",
        "Plus tard",
        "Le même rendu que sur iPhone.",
        false,
    ),
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
            ("Les étapes", "#etapes"),
            ("Les films", "#films"),
            ("Essayer en direct", "#labo"),
            ("Mac, iPhone et iPad", "#apple"),
            ("Tarifs", "#tarifs"),
        ],
    ),
    (
        "Ressources",
        &[
            ("Boîtiers pris en charge", "/docs/boitiers/"),
            ("Fonctions", "/docs/fonctions/"),
            ("Questions fréquentes", "#questions"),
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
                    "Sur Mac et Linux, LÜMA Premium s'achète uniquement sur ce site, par un abonnement payé par Stripe ; il n'est pas vendu sur le Mac App Store. Sur iPhone et iPad (iOS et iPadOS), il s'achète uniquement dans l'App Store, en un achat unique distinct de l'abonnement Mac et Linux : il est facturé par Apple, au prix affiché dans l'App Store, et se restaure sur vos autres iPhone et iPad avec le même compte Apple. Les articles Paiement et Résiliation ci‑dessous concernent l'abonnement Mac et Linux.",
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
                    "Toutes les mises à jour de LÜMA sont comprises dans LÜMA Premium, sur Mac, Linux, iPhone et iPad. Une modification des prix de l'abonnement est annoncée au moins 30 jours à l'avance et ne s'applique qu'au renouvellement suivant.",
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
