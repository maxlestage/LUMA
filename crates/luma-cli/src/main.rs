//! `luma` : le moteur de LÜMA en ligne de commande.
//!
//! ```text
//! luma info <raw>…                  ce que LÜMA lit dans chaque fichier
//! luma develop <raw> [options]      développe et exporte (JPEG, PNG, TIFF)
//! luma export-preset [nom] [opt]    les préréglages d'export : liste, crée, modifie
//! luma xmp <raw|xmp|json> [options] écrit les réglages au format XMP
//! luma bench <raw> [options]        mesure la réactivité (aperçus successifs)
//! luma merge <raw>… [options]       empilement ou panorama, en DNG
//! luma film-lut <raw> <scan>… [opt]  mesure une vraie pellicule, en LUT 3D (.cube)
//! luma presets [dossier]            exporte les 100 films en préréglages XMP
//! luma cameras [--markdown]         les boîtiers pris en charge (docs/boitiers.md)
//! luma catalog [dossier]… [opt]     la photothèque : ajoute des dossiers, liste par date
//! luma colorchecker <raw>           la fidélité des couleurs, sur la photo d'une charte
//! luma licence-keys                 une paire de clés pour signer les licences
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use luma_color::OutputSpace;
use luma_core::Cancel;
use luma_project::{Settings, xmp};
use luma_render::export::{self, Format};
use luma_render::{Photo, Size};

const USAGE: &str = "\
luma — le moteur de LÜMA

USAGE
  luma info <raw>…
  luma develop <raw> [-o sortie.jpg|.png|.tif] [--size N] [--space srgb|p3|rec2020|prophoto]
                     [--quality 1-100] [--bits 8|16] [--xmp f.xmp] [--settings f.json]
                     [--no-sidecar] [--set nom=valeur]… [--lut film.cube[:intensité]]
                     [--preset nom] [--name modèle] [--resize taille]
      Avec --preset, les options du préréglage d'export (format, qualité, espace, taille,
      dossier, nom) ; les options données les remplacent, --size est alors le grand côté
      de l'image recadrée, et la sortie va dans son dossier (LÜMA/ à côté du RAW par
      défaut). --name donne le nom du fichier par un modèle : {nom} {date} {heure}
      {année} {mois} {jour} {séquence} (ou {séquence:2}…) {boîtier} {marque} {objectif}
      {iso} {note} {libellé} ; un « / » crée un sous‑dossier ({année}/{mois}/{nom}).
      --resize : 2048 ou long:2048 (grand côté), short:1080 (petit côté), 1920x1080
      (l'image tient dans le cadre), 12mp (mégapixels) ; jamais d'agrandissement.
  luma export-preset [nom] [--format jpg|png|tif] [--quality 1-100] [--bits 8|16]
                     [--space srgb|p3|rec2020|prophoto] [--size taille|0] [--folder d]
                     [--next-to-original] [--name modèle] [--delete]
      Sans nom : la liste des préréglages d'export (Web, Pleine définition, Écran P3,
      Impression, puis les vôtres). Avec un nom : crée le préréglage ou le modifie
      (partagé avec l'app) ; --delete le retire.
  luma xmp <raw|f.xmp|f.json> [--set nom=valeur]… [-o f.xmp]
  luma bench <raw> [--size N] [--runs N]
  luma merge <raw>… [--stack mean|median|max|stars | --pano perspective|cylindrical|spherical]
                    [--no-align] [--no-crop] [--size N] [-o sortie.dng]
  luma film-lut <numérique> <scan> [<numérique> <scan>]… [-o film.cube] [--size 17-65]
                    [--aligned] [--title nom]
  luma catalog [dossier]… [--flat] [--catalog f.lumacat] [--list] [--search \"…\"] [--keywords]
               [--smart nom \"règle\"] [--add nom photo]… [--remove-collection nom]
               [--collections] [--collection nom]
      La photothèque : ajoute chaque dossier (sous‑dossiers compris, sauf avec --flat)
      au catalogue de LÜMA, ou le remet à jour ; seuls les fichiers modifiés sont relus.
      --list affiche les photos par date de prise de vue ; --search ne garde que celles
      qui répondent à la recherche : des mots (nom, boîtier, objectif, libellé,
      mots‑clés ; sans tenir compte des accents) et des filtres, par exemple
        \"mer note:3 drapeau:retenue date:2024-06..2024-08 iso:..800 focale:24..70
         boîtier:sony objectif:gm libellé:rouge mot:été\"
      --keywords compte les mots‑clés, les boîtiers et les objectifs du catalogue.
      Collections : --smart crée une collection dynamique (sa règle s'écrit comme une
      recherche : \"note:4 date:2024\"), --add range une photo dans une collection
      manuelle (créée au besoin), --collections les liste, --collection montre les
      photos de l'une d'elles.
  luma cameras [--markdown] [--corpus dossier]
      Les boîtiers pris en charge ; --markdown écrit la matrice publiée (docs/boitiers.md),
      --corpus marque ceux dont un vrai RAW est dans le dossier.
  luma colorchecker <raw> [--xmp f.xmp]
      Mesure la fidélité des couleurs d'un boîtier : photographier une charte ColorChecker
      (24 plages), la recadrer au plus juste dans LÜMA (le XMP garde le recadrage), puis
      lancer la commande : ΔE 2000 de chaque plage (moins de 2 : invisible ; moins de 5 :
      bon).
  luma licence-keys
      Une nouvelle paire de clés pour LÜMA Premium : la privée va sur le serveur
      (LUMA_LICENCE_SECRET, à garder secrète), la publique dans l'app (LUMA_LICENCE_PUBLIC).
  luma presets [dossier]
      Exporte les 100 films en préréglages XMP (.xmp), rangés par famille, dans
      « dossier/Préréglages LÜMA » (dossier courant par défaut).
      Mesure une pellicule : la même scène en numérique (RAW, développé avec les réglages
      neutres de LÜMA) et sur film (scan JPEG, PNG ou TIFF). Les images sont alignées
      automatiquement (--aligned si elles se superposent déjà). La LUT obtenue, posée
      sur n'importe quelle photo, lui donne le rendu de cette pellicule.

RÉGLAGES (--set)
  exposure contrast highlights shadows whites blacks texture clarity dehaze vibrance
  saturation temperature tint curve.highlights|lights|darks|shadows
  hsl.hue|saturation|luminance.<red|orange|yellow|green|aqua|blue|purple|magenta>
  grading.<shadows|midtones|highlights|global>.<hue|saturation|luminance>
  grading.blending grading.balance sharpen sharpen.radius sharpen.detail sharpen.masking
  noise.luminance noise.color vignette vignette.midpoint vignette.feather
  vignette.roundness grain grain.size grain.roughness crop.angle

Un fichier XMP posé à côté du RAW (IMG_0001.xmp ou IMG_0001.CR3.xmp) est lu
automatiquement, sauf avec --no-sidecar.";

struct Args {
    positional: Vec<String>,
    output: Option<PathBuf>,
    size: Option<usize>,
    space: Option<OutputSpace>,
    quality: Option<u8>,
    bits: Option<u8>,
    preset: Option<String>,
    name: Option<String>,
    resize: Option<export::Resize>,
    xmp: Option<PathBuf>,
    settings: Option<PathBuf>,
    sidecar: bool,
    sets: Vec<String>,
    runs: usize,
    lut: Option<String>,
}

fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        positional: Vec::new(),
        output: None,
        size: None,
        space: None,
        quality: None,
        bits: None,
        preset: None,
        name: None,
        resize: None,
        xmp: None,
        settings: None,
        sidecar: true,
        sets: Vec::new(),
        runs: 20,
        lut: None,
    };
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} attend une valeur"));
        match arg.as_str() {
            "-o" | "--output" => a.output = Some(value(&arg)?.into()),
            "--size" => a.size = Some(value(&arg)?.parse().map_err(|_| "--size : un nombre de pixels")?),
            "--space" => {
                let v = value(&arg)?;
                a.space = Some(OutputSpace::parse(&v).ok_or_else(|| format!("espace inconnu « {v} »"))?);
            }
            "--quality" => a.quality = Some(value(&arg)?.parse().map_err(|_| "--quality : 1 à 100")?),
            "--bits" => a.bits = Some(value(&arg)?.parse().map_err(|_| "--bits : 8 ou 16")?),
            "--preset" => a.preset = Some(value(&arg)?),
            "--name" => a.name = Some(value(&arg)?),
            "--resize" => {
                let v = value(&arg)?;
                a.resize = Some(export::Resize::parse(&v).ok_or_else(|| resize_error(&v))?);
            }
            "--xmp" => a.xmp = Some(value(&arg)?.into()),
            "--settings" => a.settings = Some(value(&arg)?.into()),
            "--no-sidecar" => a.sidecar = false,
            "--set" => a.sets.push(value(&arg)?),
            "--runs" => a.runs = value(&arg)?.parse().map_err(|_| "--runs : un nombre")?,
            "--lut" => a.lut = Some(value(&arg)?),
            "-h" | "--help" => return Err(String::new()),
            s if s.starts_with('-') => return Err(format!("option inconnue « {s} »")),
            _ => a.positional.push(arg),
        }
    }
    Ok(a)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Le sidecar XMP d'un RAW, s'il existe : `IMG_0001.xmp` ou `IMG_0001.CR3.xmp`.
fn sidecar(raw: &Path) -> Option<PathBuf> {
    let candidates = [
        raw.with_extension("xmp"),
        raw.with_extension("XMP"),
        PathBuf::from(format!("{}.xmp", raw.display())),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn load_settings(a: &Args, raw: Option<&Path>) -> Result<Settings, String> {
    let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{} : {e}", p.display()));
    let mut s = if let Some(p) = &a.settings {
        Settings::from_json(&read(p)?).map_err(|e| e.to_string())?
    } else if let Some(p) = &a.xmp {
        xmp::read(&read(p)?).map_err(|e| e.to_string())?
    } else if let Some(p) = raw.filter(|_| a.sidecar).and_then(sidecar) {
        eprintln!("réglages lus dans {}", p.display());
        xmp::read(&read(&p)?).map_err(|e| e.to_string())?
    } else {
        Settings::default()
    };
    for set in &a.sets {
        s.set(set)?;
    }
    // --lut film.cube[:intensité] : un film mesuré ou une LUT 3D.
    if let Some(spec) = &a.lut {
        let (path, amount) = match spec.rsplit_once(':') {
            Some((p, v)) if v.parse::<f32>().is_ok() => (p, v.parse().unwrap_or(100.0)),
            _ => (spec.as_str(), 100.0),
        };
        let name = luma_image::cube::load(Path::new(path)).map_err(|e| e.to_string())?;
        s.lut = Some(luma_project::Look { name, amount });
    }
    Ok(s)
}

fn catalog(args: Vec<String>) -> Result<(), String> {
    let mut file = luma_library::default_path();
    let (mut folders, mut recursive, mut show, mut facets) = (Vec::new(), true, false, false);
    let mut query = None;
    // Les changements de collections, appliqués dans l'ordre ; la collection à montrer.
    let mut edits: Vec<(String, String, String)> = Vec::new();
    let (mut list_collections, mut shown) = (false, None);
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--catalog" => file = Some(it.next().ok_or("--catalog attend un fichier")?.into()),
            "--flat" => recursive = false,
            "--list" => show = true,
            "--search" => {
                let text = it.next().ok_or("--search attend une recherche")?;
                query = Some(luma_library::Query::parse(&text)?);
            }
            "--keywords" => facets = true,
            "--smart" | "--add" => {
                let name = it.next().ok_or_else(|| format!("{arg} attend un nom de collection"))?;
                let value = it.next().ok_or_else(|| {
                    format!(
                        "{arg} {name} attend {}",
                        if arg == "--smart" { "une règle" } else { "une photo" }
                    )
                })?;
                edits.push((arg, name, value));
            }
            "--remove-collection" => {
                let name = it.next().ok_or("--remove-collection attend un nom")?;
                edits.push((arg, name, String::new()));
            }
            "--collections" => list_collections = true,
            "--collection" => shown = Some(it.next().ok_or("--collection attend un nom")?),
            s if s.starts_with('-') => return Err(format!("option inconnue « {s} »")),
            _ => folders.push(PathBuf::from(arg)),
        }
    }
    let file = file.ok_or("pas de dossier personnel pour le catalogue : --catalog f.lumacat")?;
    let t = Instant::now();
    let mut cat = luma_library::Catalog::load(&file);
    let loaded = t.elapsed();
    for dir in &folders {
        let dir = dir.canonicalize().map_err(|e| format!("{} : {e}", dir.display()))?;
        let t = Instant::now();
        let u = cat.add_folder(&dir, recursive);
        println!(
            "{} : {} nouvelles, {} relues, {} inchangées, {} retirées ({:.0} ms)",
            dir.display(),
            u.added,
            u.updated,
            u.unchanged,
            u.removed,
            ms(t.elapsed())
        );
    }
    for (action, name, value) in &edits {
        match action.as_str() {
            "--smart" => {
                cat.set_collection(luma_library::Collection::smart(name, value)?)?;
                println!("collection dynamique « {name} » : {value}");
            }
            "--add" => {
                let photo = Path::new(value).canonicalize().map_err(|e| format!("{value} : {e}"))?;
                if cat.get(&photo).is_none() {
                    return Err(format!(
                        "{} n'est pas dans le catalogue (ajouter son dossier d'abord)",
                        photo.display()
                    ));
                }
                cat.add_to_collection(name, &[photo])?;
            }
            _ if cat.remove_collection(name) => println!("collection « {name} » retirée"),
            _ => return Err(format!("pas de collection « {name} »")),
        }
    }
    if !folders.is_empty() || !edits.is_empty() {
        cat.save(&file).map_err(|e| format!("{} : {e}", file.display()))?;
    }
    println!("{} : {} photos (lu en {:.1} ms)", file.display(), cat.len(), ms(loaded));
    if facets {
        for (title, values) in [
            ("Mots‑clés", cat.keywords()),
            ("Boîtiers", cat.cameras()),
            ("Objectifs", cat.lenses()),
        ] {
            println!("{title}");
            for (v, n) in values {
                println!("  {v} ({n})");
            }
        }
    }
    if list_collections {
        for c in cat.collections() {
            let n = cat.collection_photos(&c.name).map_or(0, |v| v.len());
            match &c.content {
                luma_library::Content::Smart(rule) => println!("{} ({n}) — dynamique : {rule}", c.name),
                luma_library::Content::Manual(_) => println!("{} ({n})", c.name),
            }
        }
    }
    let found = if let Some(name) = &shown {
        Some(cat.collection_photos(name)?)
    } else if show || query.is_some() {
        let query = query.unwrap_or_default();
        let t = Instant::now();
        let found = cat.search(&query);
        if query != luma_library::Query::default() {
            println!("{} photos trouvées ({:.1} ms)", found.len(), ms(t.elapsed()));
        }
        Some(found)
    } else {
        None
    };
    for e in found.unwrap_or_default() {
        let iso = e.iso.map(|i| format!("ISO {i}")).unwrap_or_default();
        println!(
            "{}  {}  {}  {}  {}  {}",
            e.date.replace('T', " "),
            "★".repeat(e.rating.into()),
            e.camera(),
            e.lens,
            iso,
            e.path.display()
        );
    }
    Ok(())
}

fn info(a: &Args) -> Result<(), String> {
    for path in &a.positional {
        let t = Instant::now();
        let photo = Photo::open(path).map_err(|e| format!("{path} : {e}"))?;
        let raw = photo.raw();
        let wb = photo.as_shot();
        let (w, h) = photo.full_size();
        let e = &raw.exif;
        println!("{path}");
        println!("  appareil      {} {}", raw.make, raw.model);
        println!(
            "  image         {w} × {h} ({:.1} Mpx), capteur {}",
            (w * h) as f64 / 1e6,
            match raw.mosaic {
                luma_raw::Mosaic::Bayer(_) => "Bayer",
                luma_raw::Mosaic::XTrans(_) => "X-Trans",
                luma_raw::Mosaic::Linear => "RGB linéaire (DNG)",
            }
        );
        println!(
            "  balance       {:.0} K, teinte {:+.0} (telle quelle)",
            wb.temperature, wb.tint
        );
        if let Some(iso) = e.iso {
            print!("  prise de vue  ISO {iso}");
            if let Some(t) = e.exposure_time {
                if t < 1.0 {
                    print!(", 1/{:.0} s", 1.0 / t)
                } else {
                    print!(", {t:.1} s")
                }
            }
            if let Some(f) = e.aperture {
                print!(", f/{f:.1}");
            }
            if let Some(f) = e.focal_length {
                print!(", {f:.0} mm");
            }
            println!();
        }
        if let Some(lens) = &e.lens {
            println!("  objectif      {lens}");
        }
        if raw.baseline_exposure != 0.0 {
            println!("  exposition    {:+.2} IL prévus par le fichier", raw.baseline_exposure);
        }
        println!("  lecture       {:.0} ms", ms(t.elapsed()));
    }
    Ok(())
}

fn develop(a: &Args) -> Result<(), String> {
    let [path] = a.positional.as_slice() else {
        return Err("develop : un seul fichier RAW".into());
    };
    let path = Path::new(path);
    let settings = load_settings(a, Some(path))?;
    let t = Instant::now();
    let photo = Photo::open(path).map_err(|e| e.to_string())?;
    let opened = t.elapsed();
    if a.preset.is_some() || a.name.is_some() || a.resize.is_some() {
        // Un préréglage d'export (à défaut, les options par défaut) : ses options, que les
        // options données remplacent.
        let mut o = match &a.preset {
            Some(name) => {
                load_presets()?
                    .1
                    .get(name)
                    .ok_or_else(|| format!("préréglage d'export inconnu « {name} » (luma export-preset les liste)"))?
                    .options
            }
            None => export::Options::default(),
        };
        if let Some(t) = &a.name {
            export::check_template(t)?;
            o.name = Some(t.clone()).filter(|t| !t.trim().is_empty());
        }
        o.space = a.space.unwrap_or(o.space);
        o.quality = a.quality.unwrap_or(o.quality);
        o.bits = a.bits.unwrap_or(o.bits);
        o.resize = a
            .resize
            .or(a.size.filter(|&n| n > 0).map(export::Resize::LongEdge))
            .or(o.resize);
        if let Some(f) = a
            .output
            .as_deref()
            .and_then(|p| p.extension()?.to_str())
            .and_then(Format::parse)
        {
            o.format = f;
        }
        let meta = a
            .sidecar
            .then(|| sidecar(path))
            .flatten()
            .and_then(|x| std::fs::read_to_string(x).ok())
            .map(|t| xmp::read_meta(&t))
            .unwrap_or_default();
        let fields = export::NameFields::of_photo(path, &photo, &meta, 1);
        let out = a
            .output
            .clone()
            .unwrap_or_else(|| export::output_path(path, &o, &fields));
        let t2 = Instant::now();
        export::develop(&photo, &settings, &out, &o)?;
        println!("{} → {} ({})", path.display(), out.display(), export::describe(&o));
        println!("  {:<22} {:>8.1} ms", "lecture du RAW", ms(opened));
        println!("  {:<22} {:>8.1} ms", "rendu et encodage", ms(t2.elapsed()));
        return Ok(());
    }
    let size = a.size.map_or(Size::Full, Size::Fit);
    let rendered = photo
        .render(&settings, size, &Cancel::new())
        .map_err(|e| e.to_string())?;
    let out = a.output.clone().unwrap_or_else(|| path.with_extension("jpg"));
    let t2 = Instant::now();
    export::write(
        &out,
        &rendered.image,
        a.space.unwrap_or_default(),
        a.quality.unwrap_or(92),
        a.bits.unwrap_or(8),
    )?;
    println!(
        "{} → {} ({} × {})",
        path.display(),
        out.display(),
        rendered.image.width,
        rendered.image.height
    );
    println!("  {:<22} {:>8.1} ms", "lecture du RAW", ms(opened));
    println!("{}", rendered.timings);
    println!("  {:<22} {:>8.1} ms", "encodage", ms(t2.elapsed()));
    Ok(())
}

fn resize_error(v: &str) -> String {
    format!("taille inconnue « {v} » (2048, short:1080, 1920x1080, 12mp)")
}

/// Le fichier des préréglages d'export et son contenu.
fn load_presets() -> Result<(PathBuf, export::Presets), String> {
    let file = export::presets_path().ok_or("pas de dossier personnel (HOME) pour les préréglages d'export")?;
    let presets = export::Presets::load(&file);
    Ok((file, presets))
}

/// `luma export-preset` : liste, crée, modifie ou retire les préréglages d'export.
fn export_preset(args: Vec<String>) -> Result<(), String> {
    let (file, mut presets) = load_presets()?;
    let mut it = args.into_iter();
    let Some(name) = it.next().filter(|n| !n.starts_with('-')) else {
        for p in presets.all() {
            println!("{:<20} {}", p.name, export::describe(&p.options));
        }
        return Ok(());
    };
    let mut o = presets.get(&name).map(|p| p.options).unwrap_or_default();
    let mut delete = false;
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{arg} attend une valeur"));
        match arg.as_str() {
            "--format" => {
                let v = value()?;
                o.format = Format::parse(&v).ok_or_else(|| format!("format inconnu « {v} » (jpg, png, tif)"))?;
            }
            "--quality" => o.quality = value()?.parse().map_err(|_| "--quality : 1 à 100")?,
            "--bits" => {
                o.bits = match value()?.as_str() {
                    "8" => 8,
                    "16" => 16,
                    _ => return Err("--bits : 8 ou 16".into()),
                }
            }
            "--space" => {
                let v = value()?;
                o.space = OutputSpace::parse(&v).ok_or_else(|| format!("espace inconnu « {v} »"))?;
            }
            "--size" | "--resize" => {
                let v = value()?;
                o.resize = if v.trim() == "0" {
                    None
                } else {
                    Some(export::Resize::parse(&v).ok_or_else(|| resize_error(&v))?)
                };
            }
            "--folder" => o.folder = Some(value()?.into()),
            "--next-to-original" => o.folder = None,
            "--name" => {
                let t = value()?;
                export::check_template(&t)?;
                o.name = Some(t).filter(|t| !t.trim().is_empty());
            }
            "--delete" => delete = true,
            other => return Err(format!("option inconnue « {other} »")),
        }
    }
    if delete {
        if !presets.remove(&name) {
            return Err(format!("« {name} » n'est pas un de vos préréglages d'export"));
        }
        presets.save(&file)?;
        println!("« {name} » retiré");
        return Ok(());
    }
    o.quality = o.quality.clamp(1, 100);
    println!("{:<20} {}", name.trim(), export::describe(&o));
    presets.set(export::Preset {
        name: name.trim().to_owned(),
        options: o,
    });
    presets.save(&file)?;
    println!("enregistré dans {}", file.display());
    Ok(())
}

fn to_xmp(a: &Args) -> Result<(), String> {
    let [path] = a.positional.as_slice() else {
        return Err("xmp : un seul fichier".into());
    };
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut args = Args {
        positional: vec![],
        ..parse(std::iter::empty()).expect("défauts")
    };
    args.sets = a.sets.clone();
    args.sidecar = a.sidecar;
    match ext.as_str() {
        "xmp" => args.xmp = Some(p.into()),
        "json" => args.settings = Some(p.into()),
        _ => {}
    }
    let raw = (ext != "xmp" && ext != "json").then_some(p);
    let text = xmp::write(&load_settings(&args, raw)?);
    match &a.output {
        Some(out) => std::fs::write(out, text).map_err(|e| format!("{} : {e}", out.display())),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn bench(a: &Args) -> Result<(), String> {
    let [path] = a.positional.as_slice() else {
        return Err("bench : un seul fichier RAW".into());
    };
    let size = Size::Fit(a.size.unwrap_or(2048));
    let t = Instant::now();
    let photo = Photo::open(path).map_err(|e| e.to_string())?;
    let open = t.elapsed();
    let base = Settings::default();
    let t = Instant::now();
    let first = photo.render(&base, size, &Cancel::new()).map_err(|e| e.to_string())?;
    let cold = t.elapsed();
    println!(
        "{path} — aperçu {} × {}, {} cœurs",
        first.image.width,
        first.image.height,
        rayon::current_num_threads()
    );
    println!("  lecture du RAW            {:>7.1} ms", ms(open));
    println!("  premier aperçu            {:>7.1} ms", ms(cold));
    for (name, heavy) in [("réglages de base", false), ("réglages lourds", true)] {
        let mut times = Vec::new();
        for i in 0..a.runs.max(1) {
            // Un curseur qu'on fait glisser : chaque rendu a un réglage différent.
            let mut s = base.clone();
            s.exposure = -1.0 + 2.0 * i as f32 / a.runs as f32;
            if heavy {
                s.highlights = -60.0;
                s.shadows = 40.0;
                s.clarity = 25.0;
                s.texture = 15.0;
                s.dehaze = 10.0;
                s.vibrance = 20.0;
                s.hsl.saturation[5] = -20.0;
                s.color_grading.shadows.saturation = 20.0;
                s.color_grading.shadows.hue = 220.0;
                s.detail.luminance_nr = 20.0;
                s.effects.vignette_amount = -20.0;
            }
            let t = Instant::now();
            let r = photo.render(&s, size, &Cancel::new()).map_err(|e| e.to_string())?;
            let _ = luma_image::encode8(&r.image, OutputSpace::DisplayP3);
            times.push(ms(t.elapsed()));
        }
        times.sort_by(f64::total_cmp);
        println!(
            "  {name:<25} P50 {:>6.1} ms   P95 {:>6.1} ms   min {:>6.1} ms   ({} rendus + encodage P3)",
            percentile(&times, 0.5),
            percentile(&times, 0.95),
            times[0],
            times.len()
        );
    }
    Ok(())
}

/// `luma merge` : empilement (bruit, médiane, filé, étoiles) ou panorama, en DNG.
fn merge(args: Vec<String>) -> Result<(), String> {
    use luma_merge::{Projection, StackMode};
    let mut paths = Vec::new();
    let mut stack = Some(StackMode::Mean);
    let mut pano = Projection::Cylindrical;
    let (mut align, mut crop, mut size, mut out) = (true, true, Size::Full, None);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} : valeur attendue"));
        match a.as_str() {
            "--stack" => {
                stack = Some(match value()?.as_str() {
                    "mean" | "moyenne" => StackMode::Mean,
                    "median" | "mediane" => StackMode::Median,
                    "max" | "maximum" => StackMode::Maximum,
                    "stars" | "etoiles" => StackMode::Stars,
                    v => return Err(format!("empilement inconnu « {v} » (mean, median, max, stars)")),
                })
            }
            "--pano" => {
                stack = None;
                pano = match value()?.as_str() {
                    "perspective" => Projection::Perspective,
                    "cylindrical" | "cylindrique" => Projection::Cylindrical,
                    "spherical" | "spherique" => Projection::Spherical,
                    v => return Err(format!("projection inconnue « {v} »")),
                }
            }
            "--no-align" => align = false,
            "--no-crop" => crop = false,
            "--size" => size = Size::Fit(value()?.parse().map_err(|_| "--size : nombre attendu")?),
            "-o" => out = Some(PathBuf::from(value()?)),
            _ => paths.push(PathBuf::from(a)),
        }
    }
    if paths.len() < 2 {
        return Err("il faut au moins deux photos".into());
    }
    let started = Instant::now();
    let frames = luma_merge::RawFiles {
        paths: paths.clone(),
        size,
    };
    let progress = |f: f32, label: &str| eprint!("\r  {label:<32} {:>3.0} %", f * 100.0);
    let cancel = Cancel::new();
    let (merged, suffix) = match stack {
        Some(mode) => (
            luma_merge::stack(&frames, mode, align, &progress, &cancel).map_err(|e| e.to_string())?,
            mode.name(),
        ),
        None => (
            luma_merge::panorama(&frames, pano, &progress, &cancel).map_err(|e| e.to_string())?,
            "Pano",
        ),
    };
    eprintln!();
    let template = luma_raw::RawPhoto::open(&paths[paths.len() / 2]).map_err(|e| e.to_string())?;
    let out = out.unwrap_or_else(|| luma_merge::output_path(&paths[0], &suffix.replace('é', "e")));
    merged.save(&out, &template, crop).map_err(|e| e.to_string())?;
    println!(
        "{} ({} × {}, {} photo(s){}) en {:.1} s",
        out.display(),
        merged.image.width,
        merged.image.height,
        merged.used,
        if merged.skipped.is_empty() {
            String::new()
        } else {
            format!(", {} écartée(s)", merged.skipped.len())
        },
        started.elapsed().as_secs_f32()
    );
    if let Some(c) = merged.crop {
        println!(
            "  rognage {} : {:.1} % × {:.1} % de l'image",
            if crop {
                "proposé dans le XMP"
            } else {
                "non appliqué (--no-crop)"
            },
            (c.right - c.left) * 100.0,
            (c.bottom - c.top) * 100.0
        );
    }
    Ok(())
}

/// `luma colorchecker` : la fidélité des couleurs sur la photo d'une vraie charte.
fn colorchecker(a: &Args) -> Result<(), String> {
    let path = a
        .positional
        .first()
        .ok_or("un RAW de la charte, recadré au plus juste")?;
    let raw = Path::new(path);
    let mut s = load_settings(a, Some(raw))?;
    // La couleur seule : pas de look, pas de courbe (l'étape d'entrée et la balance).
    let keep = (s.white_balance, s.crop, s.transform.clone(), s.lens.clone());
    s = Settings::default();
    (s.white_balance, s.crop, s.transform, s.lens) = keep;
    let photo = Photo::open(raw).map_err(|e| e.to_string())?;
    let mut t = luma_core::Timings::default();
    let base = photo
        .base(&s, Size::Fit(1600), &Cancel::new(), &mut t)
        .map_err(|e| e.to_string())?;
    let de = luma_render::chart::measure_chart(&base.image);
    println!(
        "{} {} — ΔE 2000 par plage (charte ColorChecker)",
        photo.raw().make,
        photo.raw().model
    );
    for row in 0..4 {
        let line: Vec<String> = (0..6).map(|c| format!("{:5.1}", de[row * 6 + c])).collect();
        println!("  {}", line.join(" "));
    }
    let mean = de.iter().sum::<f32>() / 24.0;
    let worst = de.iter().copied().fold(0.0, f32::max);
    println!(
        "moyenne {mean:.2}, maximum {worst:.2} : {}",
        if mean < 2.0 {
            "couleurs fidèles (écart invisible)"
        } else if mean < 5.0 {
            "bonne fidélité"
        } else {
            "écart visible : vérifier le recadrage, la balance des blancs et l'éclairage"
        }
    );
    Ok(())
}

/// `luma presets` : les films intégrés en préréglages XMP, un dossier par famille.
fn export_presets(dir: &Path) -> Result<usize, String> {
    use luma_project::presets::PRESETS;
    for p in PRESETS {
        let folder = dir.join("Préréglages LÜMA").join(p.group.name());
        std::fs::create_dir_all(&folder).map_err(|e| format!("{} : {e}", folder.display()))?;
        let file = folder.join(format!("{}.xmp", p.name.replace(['/', ':'], "-")));
        let text = xmp::write_preset(p.name, &format!("LÜMA — {}", p.group.name()), &p.look());
        std::fs::write(&file, text).map_err(|e| format!("{} : {e}", file.display()))?;
    }
    Ok(PRESETS.len())
}

/// `luma film-lut` : mesure une vraie pellicule en LUT 3D.
fn film_lut(args: Vec<String>) -> Result<(), String> {
    use luma_merge::film;
    let (mut out, mut size, mut aligned, mut title) = (None, 33, false, None);
    let mut paths = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} : valeur attendue"));
        match a.as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(value()?)),
            "--size" => size = value()?.parse().map_err(|_| "--size : 17 à 65")?,
            "--aligned" => aligned = true,
            "--title" => title = Some(value()?),
            s if s.starts_with('-') => return Err(format!("option inconnue « {s} »")),
            _ => paths.push(PathBuf::from(a)),
        }
    }
    if paths.is_empty() || paths.len() % 2 != 0 {
        return Err("il faut des paires : <numérique> <scan> [<numérique> <scan>]…".into());
    }
    let started = Instant::now();
    let pairs = paths
        .chunks(2)
        .map(|p| {
            Ok(film::Pair {
                digital: film::load_reference(&p[0]).map_err(|e| e.to_string())?,
                film: film::load_scan(&p[1]).map_err(|e| e.to_string())?,
                aligned,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let out = out.unwrap_or_else(|| paths[1].with_extension("cube"));
    let title = title.unwrap_or_else(|| {
        out.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let m = film::measure(&pairs, size, &title).map_err(|e| e.to_string())?;
    std::fs::write(&out, m.cube.to_text()).map_err(|e| format!("{} : {e}", out.display()))?;
    println!(
        "{} : LUT {n}×{n}×{n} mesurée sur {} pixels en {:.1} s",
        out.display(),
        m.samples,
        started.elapsed().as_secs_f32(),
        n = m.cube.size
    );
    println!(
        "  écart numérique → pellicule : {:.2} ; avec la LUT : {:.2} (OKLab ×100, 1 ≈ à peine visible)",
        m.before, m.after
    );
    Ok(())
}

fn main() -> ExitCode {
    // HEIC, AVIF… : le décodeur d'images du système (ImageIO sur Mac).
    luma_apple::install();
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    };
    if command == "licence-keys" {
        let mut seed = [0u8; 32];
        match std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut seed)) {
            Ok(()) => {
                let (secret, public) = luma_licence::keypair(seed);
                println!("LUMA_LICENCE_SECRET={secret}");
                println!("LUMA_LICENCE_PUBLIC={public}");
                eprintln!(
                    "La première ligne va sur le serveur (heroku config:set), et nulle part ailleurs ;\nla seconde dans les variables du dépôt (Actions), pour construire l'app."
                );
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("erreur : pas de hasard ({e})");
                return ExitCode::FAILURE;
            }
        }
    }
    if command == "cameras" {
        let args: Vec<String> = args.collect();
        let corpus = args
            .iter()
            .position(|a| a == "--corpus")
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from);
        // Les boîtiers du corpus : lus dans les vrais fichiers s'ils sont là.
        let mut tested: Vec<(String, String)> = luma_raw::cameras::CORPUS
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect();
        if let Some(dir) = corpus
            && let Ok(entries) = std::fs::read_dir(&dir)
        {
            for e in entries.flatten() {
                if let Ok(raw) = luma_raw::RawPhoto::open(e.path())
                    && !tested.iter().any(|(m, d)| *m == raw.make && *d == raw.model)
                {
                    tested.push((raw.make, raw.model));
                }
            }
        }
        let tested: Vec<(&str, &str)> = tested.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        if args.iter().any(|a| a == "--markdown") {
            print!("{}", luma_raw::cameras::markdown(&tested));
        } else {
            for c in luma_raw::cameras::all() {
                println!("{}\t{}\t{}", c.make, c.model, if c.color { "couleur" } else { "" });
            }
        }
        return ExitCode::SUCCESS;
    }
    if command == "presets" {
        let dir = PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
        return match export_presets(&dir) {
            Ok(n) => {
                println!(
                    "{n} préréglages exportés dans {}",
                    dir.join("Préréglages LÜMA").display()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "catalog" {
        return match catalog(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "export-preset" {
        return match export_preset(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "film-lut" {
        return match film_lut(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "merge" {
        return match merge(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    let parsed = match parse(args) {
        Ok(a) => a,
        Err(e) if e.is_empty() => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("erreur : {e}");
            return ExitCode::from(2);
        }
    };
    let result = match command.as_str() {
        "info" => info(&parsed),
        "develop" => develop(&parsed),
        "xmp" => to_xmp(&parsed),
        "colorchecker" => colorchecker(&parsed),
        "bench" => bench(&parsed),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        other => Err(format!("commande inconnue « {other} »\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erreur : {e}");
            ExitCode::FAILURE
        }
    }
}
