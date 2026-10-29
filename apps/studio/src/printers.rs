//! Les imprimantes du système, et l'envoi d'un PDF à l'une d'elles, sans passer par
//! le lecteur de PDF : sur Mac et Linux, par le service d'impression du système
//! (CUPS, ses commandes `lpstat` et `lp`). Ailleurs (Windows, iPhone, iPad), aucune
//! imprimante n'est listée : le PDF s'ouvre et s'imprime depuis son lecteur.

use std::path::Path;
use std::process::Command;

/// Le service d'impression est‑il là où l'app tourne ?
pub const AVAILABLE: bool = cfg!(unix) && !cfg!(target_os = "ios");

/// Les imprimantes du système, la sienne par défaut d'abord ; vide sans service
/// d'impression (ou sans imprimante).
pub fn list() -> Vec<String> {
    if !AVAILABLE {
        return Vec::new();
    }
    let run = |args: &[&str]| {
        Command::new("lpstat")
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    // `lpstat -e` (les noms seuls) n'existe pas partout : `lpstat -a` sinon.
    let mut names = destinations(&run(&["-e"]));
    if names.is_empty() {
        names = destinations(&run(&["-a"]));
    }
    with_default_first(names, default_destination(&run(&["-d"])).as_deref())
}

/// Les noms d'imprimantes d'une sortie de `lpstat -e` (un nom par ligne) ou de
/// `lpstat -a` (le nom, puis son état), sans doublon.
pub fn destinations(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in text.lines().filter_map(|l| l.split_whitespace().next()) {
        if !names.iter().any(|n| n == name) {
            names.push(name.to_owned());
        }
    }
    names
}

/// L'imprimante par défaut d'une sortie de `lpstat -d`
/// (« system default destination: Bureau »), s'il y en a une.
pub fn default_destination(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.split_once("default destination:"))
        .map(|(_, name)| name.trim().to_owned())
        .filter(|n| !n.is_empty())
}

/// `names`, l'imprimante par défaut en tête.
fn with_default_first(mut names: Vec<String>, default: Option<&str>) -> Vec<String> {
    if let Some(i) = default.and_then(|d| names.iter().position(|n| n == d)) {
        let d = names.remove(i);
        names.insert(0, d);
    }
    names
}

/// Les arguments de `lp` : l'imprimante (sinon celle par défaut), le nombre
/// d'exemplaires (1 à 99), puis le fichier, après `--` (un nom qui commencerait par un
/// tiret n'est pas pris pour une option).
pub fn arguments(file: &Path, printer: Option<&str>, copies: u32) -> Vec<String> {
    let mut a = Vec::new();
    if let Some(p) = printer {
        a.extend(["-d".to_owned(), p.to_owned()]);
    }
    a.extend(["-n".to_owned(), copies.clamp(1, 99).to_string()]);
    a.extend(["--".to_owned(), file.display().to_string()]);
    a
}

/// Envoie `file` à l'imprimante ; rend le numéro de la demande que donne le système
/// (« Bureau-12 »), ou son message d'erreur.
pub fn send(file: &Path, printer: Option<&str>, copies: u32) -> Result<String, String> {
    send_with("lp", file, printer, copies)
}

/// [`send`] par le programme `program` (les essais en donnent un à eux).
pub fn send_with(
    program: &str,
    file: &Path,
    printer: Option<&str>,
    copies: u32,
) -> Result<String, String> {
    let out = Command::new(program)
        .args(arguments(file, printer, copies))
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        return Err(if err.is_empty() {
            text.trim().to_owned()
        } else {
            err
        });
    }
    // « request id is Bureau-12 (1 file(s)) »
    Ok(text
        .split_once("request id is")
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .unwrap_or("")
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printers_are_read_from_the_system_answers() {
        assert_eq!(
            destinations("Bureau\nSalon_Photo\n\nBureau\n"),
            ["Bureau", "Salon_Photo"]
        );
        assert_eq!(
            destinations(
                "Bureau accepting requests since Thu 29 Oct 2026\nSalon accepting requests since …\n"
            ),
            ["Bureau", "Salon"]
        );
        assert_eq!(
            default_destination("system default destination: Salon\n").as_deref(),
            Some("Salon")
        );
        assert_eq!(default_destination("no system default destination\n"), None);
        assert_eq!(
            with_default_first(vec!["A".into(), "B".into(), "C".into()], Some("C")),
            ["C", "A", "B"]
        );
        assert_eq!(with_default_first(vec!["A".into()], Some("Z")), ["A"]);
    }

    #[test]
    fn the_print_command_names_printer_copies_and_file() {
        let f = Path::new("/tmp/-tirage.pdf");
        assert_eq!(
            arguments(f, Some("Bureau"), 3),
            ["-d", "Bureau", "-n", "3", "--", "/tmp/-tirage.pdf"]
        );
        assert_eq!(arguments(f, None, 0), ["-n", "1", "--", "/tmp/-tirage.pdf"]);
        assert_eq!(arguments(f, None, 500)[1], "99");
    }

    /// Un faux `lp` : il note ses arguments et répond comme le système ; puis un qui
    /// refuse : son message revient.
    #[cfg(unix)]
    #[test]
    fn a_pdf_is_sent_and_the_request_number_comes_back() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("luma-lp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = |name: &str, body: &str| {
            let p = dir.join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p.display().to_string()
        };
        let noted = dir.join("args.txt");
        let ok = script(
            "lp",
            &format!(
                "printf '%s\\n' \"$@\" > '{}'\necho 'request id is Bureau-12 (1 file(s))'",
                noted.display()
            ),
        );
        let pdf = dir.join("Tirage 1.pdf");
        assert_eq!(
            send_with(&ok, &pdf, Some("Bureau"), 2).as_deref(),
            Ok("Bureau-12")
        );
        let args = std::fs::read_to_string(&noted).unwrap();
        assert_eq!(args, format!("-d\nBureau\n-n\n2\n--\n{}\n", pdf.display()));
        let refused = script(
            "lp-refus",
            "echo 'lp: The printer or class does not exist.' >&2\nexit 1",
        );
        assert_eq!(
            send_with(&refused, &pdf, Some("Absente"), 1),
            Err("lp: The printer or class does not exist.".to_owned())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
