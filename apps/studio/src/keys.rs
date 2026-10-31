//! Les raccourcis clavier, personnalisables : chaque commande de l'app a ses touches
//! par défaut (celles de toujours), que l'on peut changer ; seuls les changements sont
//! enregistrés, dans « Raccourcis.txt », une ligne par commande :
//!
//! ```text
//! mask_brush = M
//! mask_linear =
//! undo = Cmd+Z, Cmd+Y
//! ```
//!
//! Une combinaison s'écrit « Cmd+Shift+Z » dans le fichier (des noms qui ne dépendent
//! pas de la langue) et se montre traduite (« Cmd+Maj+Z »).

use std::collections::BTreeMap;
use std::path::Path;

use egui::{InputState, Key, Modifiers};

use crate::i18n::tr;

/// Une combinaison : une touche et ses modificateurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Combo {
    pub key: Key,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Combo {
    pub const fn new(key: Key) -> Combo {
        Combo {
            key,
            command: false,
            shift: false,
            alt: false,
        }
    }

    const fn cmd(key: Key) -> Combo {
        Combo {
            command: true,
            ..Combo::new(key)
        }
    }

    const fn shift(key: Key) -> Combo {
        Combo {
            shift: true,
            ..Combo::new(key)
        }
    }

    const fn cmd_shift(key: Key) -> Combo {
        Combo {
            command: true,
            shift: true,
            ..Combo::new(key)
        }
    }

    pub fn modifiers(&self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.command {
            m |= Modifiers::COMMAND;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        m
    }

    /// Le nombre de modificateurs : les combinaisons les plus précises sont essayées
    /// d'abord (Maj+M avant M, que Maj n'empêcherait pas).
    fn weight(&self) -> u8 {
        u8::from(self.command) + u8::from(self.shift) + u8::from(self.alt)
    }

    /// Le nom dans le fichier : « Cmd+Shift+Z ».
    pub fn id(&self) -> String {
        let mut parts = Vec::new();
        if self.command {
            parts.push("Cmd");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.name());
        parts.join("+")
    }

    pub fn parse(s: &str) -> Option<Combo> {
        let mut c = Combo::new(Key::A);
        let mut key = None;
        for part in s.split('+').map(str::trim) {
            match part {
                "Cmd" | "Ctrl" => c.command = true,
                "Shift" => c.shift = true,
                "Alt" => c.alt = true,
                k => key = Some(Key::from_name(k)?),
            }
        }
        c.key = key?;
        Some(c)
    }

    /// Le nom montré, traduit : « Cmd+Maj+Z », « Espace », « Flèche gauche ».
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.command {
            parts.push(
                if cfg!(target_os = "macos") {
                    "Cmd"
                } else {
                    "Ctrl"
                }
                .into(),
            );
        }
        if self.shift {
            parts.push(tr("Maj").into());
        }
        if self.alt {
            parts.push(tr("Option").into());
        }
        let key = match self.key {
            Key::Space => tr("Espace").to_owned(),
            Key::Enter => tr("Entrée").to_owned(),
            Key::Escape => tr("Échap").to_owned(),
            Key::Tab => tr("Tab").to_owned(),
            Key::Delete => tr("Suppr").to_owned(),
            Key::Backspace => tr("Retour arrière").to_owned(),
            Key::ArrowLeft => tr("Flèche gauche").to_owned(),
            Key::ArrowRight => tr("Flèche droite").to_owned(),
            Key::ArrowUp => tr("Flèche haut").to_owned(),
            Key::ArrowDown => tr("Flèche bas").to_owned(),
            Key::Backslash => "\\".to_owned(),
            k => k.symbol_or_name().to_owned(),
        };
        parts.push(key);
        parts.join("+")
    }
}

/// Une commande : son nom pour l'app, son nom montré (en français, traduit à
/// l'affichage), ses touches par défaut ; `always` : même pendant la saisie d'un texte
/// (les combinaisons avec Cmd).
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub always: bool,
    pub defaults: &'static [Combo],
}

const fn a(id: &'static str, label: &'static str, defaults: &'static [Combo]) -> Action {
    Action {
        id,
        label,
        always: false,
        defaults,
    }
}

const fn cmd(id: &'static str, label: &'static str, defaults: &'static [Combo]) -> Action {
    Action {
        id,
        label,
        always: true,
        defaults,
    }
}

/// Toutes les commandes, dans l'ordre de la fenêtre des raccourcis. « space » : la
/// photo suivante, ou la pause pendant le diaporama.
pub const ACTIONS: &[Action] = &[
    cmd("undo", "Annuler", &[Combo::cmd(Key::Z)]),
    cmd("redo", "Rétablir", &[Combo::cmd_shift(Key::Z)]),
    cmd("copy", "Copier les réglages", &[Combo::cmd_shift(Key::C)]),
    cmd("paste", "Coller les réglages", &[Combo::cmd_shift(Key::V)]),
    cmd(
        "paste_some",
        "Coller certains réglages",
        &[Combo {
            alt: true,
            ..Combo::cmd_shift(Key::V)
        }],
    ),
    cmd(
        "sync",
        "Synchroniser les photos choisies",
        &[Combo::cmd_shift(Key::S)],
    ),
    cmd(
        "reset",
        "Réinitialiser la photo",
        &[Combo::cmd_shift(Key::R)],
    ),
    cmd("export", "Exporter", &[Combo::cmd_shift(Key::E)]),
    cmd("open", "Ouvrir", &[Combo::cmd(Key::O)]),
    a("library", "Bibliothèque", &[Combo::new(Key::G)]),
    a(
        "develop",
        "Développement",
        &[Combo::new(Key::D), Combo::new(Key::E)],
    ),
    a("culling", "Tri en plein écran", &[Combo::new(Key::F)]),
    a("compare", "Comparer", &[Combo::new(Key::C)]),
    a("loupe", "Loupe de la comparaison", &[Combo::new(Key::L)]),
    a("stack", "Déplier ou replier la pile", &[Combo::new(Key::S)]),
    a("prev", "Photo précédente", &[Combo::new(Key::ArrowLeft)]),
    a("next", "Photo suivante", &[Combo::new(Key::ArrowRight)]),
    a(
        "space",
        "Photo suivante, pause du diaporama",
        &[Combo::new(Key::Space)],
    ),
    a("auto_advance", "Avance automatique", &[Combo::new(Key::A)]),
    a("rate0", "Sans note", &[Combo::new(Key::Num0)]),
    a("rate1", "Une étoile", &[Combo::new(Key::Num1)]),
    a("rate2", "Deux étoiles", &[Combo::new(Key::Num2)]),
    a("rate3", "Trois étoiles", &[Combo::new(Key::Num3)]),
    a("rate4", "Quatre étoiles", &[Combo::new(Key::Num4)]),
    a("rate5", "Cinq étoiles", &[Combo::new(Key::Num5)]),
    a("pick", "Retenue", &[Combo::new(Key::P)]),
    a("reject", "Rejetée", &[Combo::new(Key::X)]),
    a("unflag", "Sans drapeau", &[Combo::new(Key::U)]),
    a("label0", "Libellé rouge", &[Combo::new(Key::Num6)]),
    a("label1", "Libellé jaune", &[Combo::new(Key::Num7)]),
    a("label2", "Libellé vert", &[Combo::new(Key::Num8)]),
    a("label3", "Libellé bleu", &[Combo::new(Key::Num9)]),
    a("label4", "Libellé violet", &[]),
    a("before", "Avant / après", &[Combo::new(Key::Backslash)]),
    a("split", "Avant / après côte à côte", &[Combo::new(Key::Y)]),
    a(
        "reference",
        "Photo de référence à côté",
        &[Combo {
            shift: true,
            ..Combo::new(Key::Y)
        }],
    ),
    a("zoom", "Zoom 100 %", &[Combo::new(Key::Z)]),
    a("clipping", "Écrêtage", &[Combo::new(Key::J)]),
    a("scope", "Instruments de mesure", &[Combo::shift(Key::J)]),
    a("crop", "Recadrer", &[Combo::new(Key::R)]),
    a("auto_level", "Redresser l'horizon", &[Combo::shift(Key::R)]),
    a(
        "auto_vertical",
        "Redresser les verticales",
        &[Combo::shift(Key::V)],
    ),
    a(
        "photo_guide",
        "Grille de composition sur la photo",
        &[Combo::new(Key::H)],
    ),
    a(
        "guide_turn",
        "Tourner la grille de composition",
        &[Combo::shift(Key::O)],
    ),
    a(
        "crop_done",
        "Valider le recadrage",
        &[Combo::new(Key::Enter)],
    ),
    a("crop_cancel", "Annuler, sortir", &[Combo::new(Key::Escape)]),
    a("mask_linear", "Masque linéaire", &[Combo::new(Key::M)]),
    a("mask_radial", "Masque radial", &[Combo::shift(Key::M)]),
    a("mask_brush", "Pinceau", &[Combo::new(Key::K)]),
    a("mask_overlay", "Montrer le masque", &[Combo::new(Key::O)]),
    a(
        "mask_delete",
        "Supprimer le masque",
        &[Combo::new(Key::Delete), Combo::new(Key::Backspace)],
    ),
    a("retouch", "Retouche des taches", &[Combo::new(Key::Q)]),
    a("eyedropper", "Pipette de la balance", &[Combo::new(Key::W)]),
    a("targeting", "Réglage ciblé", &[Combo::new(Key::T)]),
    a("panels", "Masquer les panneaux", &[Combo::new(Key::Tab)]),
    cmd(
        "workspace1",
        "Espace de travail 1",
        &[Combo::cmd(Key::Num1)],
    ),
    cmd(
        "workspace2",
        "Espace de travail 2",
        &[Combo::cmd(Key::Num2)],
    ),
    cmd(
        "workspace3",
        "Espace de travail 3",
        &[Combo::cmd(Key::Num3)],
    ),
    cmd(
        "workspace4",
        "Espace de travail 4",
        &[Combo::cmd(Key::Num4)],
    ),
    cmd(
        "workspace5",
        "Espace de travail 5",
        &[Combo::cmd(Key::Num5)],
    ),
    cmd(
        "workspace6",
        "Espace de travail 6",
        &[Combo::cmd(Key::Num6)],
    ),
    cmd(
        "workspace7",
        "Espace de travail 7",
        &[Combo::cmd(Key::Num7)],
    ),
    cmd(
        "workspace8",
        "Espace de travail 8",
        &[Combo::cmd(Key::Num8)],
    ),
    cmd(
        "workspace9",
        "Espace de travail 9",
        &[Combo::cmd(Key::Num9)],
    ),
];

/// Les touches de chaque commande.
#[derive(Clone, Debug, PartialEq)]
pub struct Keymap {
    keys: BTreeMap<&'static str, Vec<Combo>>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap {
            keys: ACTIONS
                .iter()
                .map(|a| (a.id, a.defaults.to_vec()))
                .collect(),
        }
    }
}

impl Keymap {
    /// Lit le fichier ; une commande ou une touche inconnue est ignorée.
    pub fn load(path: &Path) -> Keymap {
        let mut map = Keymap::default();
        let text = std::fs::read_to_string(path).unwrap_or_default();
        for line in text.lines() {
            let Some((id, combos)) = line.split_once('=') else {
                continue;
            };
            if let Some(a) = ACTIONS.iter().find(|a| a.id == id.trim()) {
                let combos = combos
                    .split(',')
                    .filter_map(|c| Combo::parse(c.trim()))
                    .collect();
                map.keys.insert(a.id, combos);
            }
        }
        map
    }

    /// Écrit les commandes qui ne sont plus à leurs touches par défaut.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut text = String::new();
        for a in ACTIONS {
            let now = self.get(a.id);
            if now != a.defaults {
                let combos: Vec<String> = now.iter().map(Combo::id).collect();
                text.push_str(&format!("{} = {}\n", a.id, combos.join(", ")));
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }

    pub fn get(&self, id: &str) -> &[Combo] {
        self.keys.get(id).map_or(&[], Vec::as_slice)
    }

    /// Donne cette seule combinaison à la commande ; la retire à celle qui l'avait (rend
    /// son nom).
    pub fn set(&mut self, id: &'static str, combo: Combo) -> Option<&'static str> {
        let before = self.conflict(combo, id);
        for combos in self.keys.values_mut() {
            combos.retain(|c| *c != combo);
        }
        self.keys.insert(id, vec![combo]);
        before
    }

    /// Retire toutes les touches de la commande.
    pub fn clear(&mut self, id: &'static str) {
        self.keys.insert(id, Vec::new());
    }

    /// Les touches par défaut, pour une commande ou toutes.
    pub fn reset(&mut self, id: Option<&'static str>) {
        match id {
            Some(id) => {
                let defaults = ACTIONS
                    .iter()
                    .find(|a| a.id == id)
                    .map_or(&[][..], |a| a.defaults);
                for c in defaults {
                    for combos in self.keys.values_mut() {
                        combos.retain(|x| x != c);
                    }
                }
                self.keys.insert(id, defaults.to_vec());
            }
            None => *self = Keymap::default(),
        }
    }

    /// La commande (autre que `except`) qui a déjà cette combinaison.
    pub fn conflict(&self, combo: Combo, except: &str) -> Option<&'static str> {
        self.keys
            .iter()
            .find(|(id, combos)| **id != except && combos.contains(&combo))
            .map(|(id, _)| *id)
    }

    /// La commande des touches pressées à cette image, s'il y en a une (la combinaison
    /// est alors consommée) ; pendant la saisie d'un texte, seulement celles avec Cmd.
    pub fn find(&self, i: &mut InputState, typing: bool) -> Option<&'static str> {
        let mut all: Vec<(Combo, &Action)> = ACTIONS
            .iter()
            .filter(|a| a.always || !typing)
            .flat_map(|a| self.get(a.id).iter().map(move |c| (*c, a)))
            .collect();
        // Les plus précises d'abord, dans l'ordre de la liste à égalité.
        all.sort_by_key(|(c, _)| std::cmp::Reverse(c.weight()));
        all.into_iter()
            .find(|(c, _)| i.consume_key(c.modifiers(), c.key))
            .map(|(_, a)| a.id)
    }
}

/// Le nom montré d'une commande, traduit.
pub fn label(id: &str) -> &'static str {
    ACTIONS
        .iter()
        .find(|a| a.id == id)
        .map_or("", |a| tr(a.label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_by_name() {
        for c in [
            Combo::cmd_shift(Key::Z),
            Combo::new(Key::Space),
            Combo::shift(Key::M),
            Combo {
                alt: true,
                ..Combo::cmd_shift(Key::V)
            },
            Combo::new(Key::ArrowLeft),
            Combo::new(Key::Num6),
        ] {
            assert_eq!(Combo::parse(&c.id()), Some(c), "{}", c.id());
        }
        assert_eq!(Combo::cmd_shift(Key::Z).id(), "Cmd+Shift+Z");
        assert!(Combo::parse("Cmd+Inconnue").is_none() && Combo::parse("Shift").is_none());
    }

    #[test]
    fn no_default_key_is_used_twice() {
        let mut seen = std::collections::HashMap::new();
        for a in ACTIONS {
            for c in a.defaults {
                assert!(
                    seen.insert(*c, a.id).is_none(),
                    "{} : {} et {}",
                    c.id(),
                    a.id,
                    seen[c]
                );
            }
        }
    }

    #[test]
    fn changed_keys_are_saved_alone_and_conflicts_resolved() {
        let mut k = Keymap::default();
        // Donner M au pinceau : le masque linéaire la perd.
        assert_eq!(k.set("mask_brush", Combo::new(Key::M)), Some("mask_linear"));
        assert!(k.get("mask_linear").is_empty());
        assert_eq!(k.conflict(Combo::new(Key::M), "rien"), Some("mask_brush"));
        let path = std::env::temp_dir().join(format!("luma-raccourcis-{}.txt", std::process::id()));
        k.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("mask_brush = M\n") && text.contains("mask_linear = \n"),
            "{text}"
        );
        assert!(!text.contains("undo"), "seulement ce qui a changé");
        assert_eq!(Keymap::load(&path), k);
        k.reset(Some("mask_linear"));
        assert_eq!(k.get("mask_linear"), [Combo::new(Key::M)]);
        assert!(
            k.get("mask_brush").is_empty(),
            "la touche revient à sa commande"
        );
        k.reset(None);
        assert_eq!(k, Keymap::default());
        let _ = std::fs::remove_file(&path);
        assert_eq!(Keymap::load(&path), Keymap::default());
    }

    #[test]
    fn every_command_name_is_translated() {
        let known: std::collections::HashSet<&str> = crate::i18n::SOURCE
            .lines()
            .filter_map(|l| l.strip_prefix("fr = "))
            .collect();
        for a in ACTIONS {
            assert!(
                known.contains(a.label),
                "« {} » sans traduction dans i18n.txt",
                a.label
            );
        }
    }
}
