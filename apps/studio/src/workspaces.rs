//! Les espaces de travail : l'outil ouvert (donc la Bibliothèque ou le Développement)
//! et les panneaux montrés ou masqués, sous un nom. Quatre sont toujours là ; les
//! autres sont ceux qu'on enregistre, dans « Espaces de travail.txt », une ligne par
//! espace :
//!
//! ```text
//! Portraits = Masks, panneaux
//! Écran seul = Light, sans panneaux
//! ```
//!
//! Cmd+1 à Cmd+9 passent d'un espace à l'autre, dans l'ordre de la liste.

use std::path::Path;

use crate::app::Tool;

#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub name: String,
    pub tool: Tool,
    pub panels: bool,
}

/// Les espaces de toujours (leurs noms sont traduits à l'affichage).
pub fn builtin() -> Vec<Workspace> {
    let w = |name: &str, tool, panels| Workspace {
        name: name.into(),
        tool,
        panels,
    };
    vec![
        w("Tri", Tool::Library, true),
        w("Développement", Tool::Light, true),
        w("Retouche", Tool::Masks, true),
        w("Présentation", Tool::Light, false),
    ]
}

/// Le nom d'un outil dans le fichier.
fn tool_id(t: Tool) -> String {
    format!("{t:?}")
}

fn tool_of(id: &str) -> Option<Tool> {
    Tool::ALL.into_iter().find(|t| tool_id(*t) == id)
}

/// Les espaces enregistrés ; une ligne illisible est ignorée.
pub fn load(path: &Path) -> Vec<Workspace> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once('=')?;
            let (tool, panels) = rest.split_once(',')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some(Workspace {
                name: name.to_owned(),
                tool: tool_of(tool.trim())?,
                panels: panels.trim() != "sans panneaux",
            })
        })
        .collect()
}

pub fn save(path: &Path, list: &[Workspace]) -> std::io::Result<()> {
    let text: String = list
        .iter()
        .map(|w| {
            let panels = if w.panels {
                "panneaux"
            } else {
                "sans panneaux"
            };
            // Ni « = » ni retour à la ligne dans un nom : le fichier se relit tel quel.
            let name = w.name.replace(['=', '\n', '\r'], " ");
            format!("{} = {}, {panels}\n", name.trim(), tool_id(w.tool))
        })
        .collect();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Ajoute un espace, ou remplace celui du même nom.
pub fn put(list: &mut Vec<Workspace>, w: Workspace) {
    match list.iter_mut().find(|x| x.name == w.name) {
        Some(x) => *x = w,
        None => list.push(w),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_and_read_back() {
        let path = std::env::temp_dir().join(format!("luma-espaces-{}.txt", std::process::id()));
        let mut list = Vec::new();
        put(
            &mut list,
            Workspace {
                name: "Portraits".into(),
                tool: Tool::Masks,
                panels: true,
            },
        );
        put(
            &mut list,
            Workspace {
                name: "Écran = seul".into(),
                tool: Tool::Color,
                panels: false,
            },
        );
        put(
            &mut list,
            Workspace {
                name: "Portraits".into(),
                tool: Tool::Detail,
                panels: true,
            },
        );
        assert_eq!(list.len(), 2, "le même nom remplace");
        save(&path, &list).unwrap();
        let back = load(&path);
        assert_eq!(back[0], list[0]);
        assert_eq!(
            (back[1].name.as_str(), back[1].tool, back[1].panels),
            ("Écran   seul", Tool::Color, false)
        );
        std::fs::write(
            &path,
            "n'importe quoi\nX = Inconnu, panneaux\n = Light, panneaux\n",
        )
        .unwrap();
        assert!(load(&path).is_empty());
        let _ = std::fs::remove_file(&path);
        assert_eq!(builtin().len(), 4);
    }
}
