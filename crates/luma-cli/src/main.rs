//! `luma` : le moteur de LÜMA en ligne de commande.
//!
//! ```text
//! luma info <raw>…                  ce que LÜMA lit dans chaque fichier
//! luma develop <raw> [options]      développe et exporte (JPEG, PNG, TIFF)
//! luma xmp <raw|xmp|json> [options] écrit les réglages au format XMP de Lightroom
//! luma bench <raw> [options]        mesure la réactivité (aperçus successifs)
//! luma merge <raw>… [options]       empilement ou panorama, en DNG
//! luma film-lut <raw> <scan>… [opt]  mesure une vraie pellicule, en LUT 3D (.cube)
//! luma presets [dossier]            exporte les 100 films en préréglages Lightroom
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use luma_color::OutputSpace;
use luma_core::Cancel;
use luma_project::{Settings, xmp};
use luma_render::{Photo, Size};

const USAGE: &str = "\
luma — le moteur de LÜMA

USAGE
  luma info <raw>…
  luma develop <raw> [-o sortie.jpg|.png|.tif] [--size N] [--space srgb|p3|rec2020|prophoto]
                     [--quality 1-100] [--bits 8|16] [--xmp f.xmp] [--settings f.json]
                     [--no-sidecar] [--set nom=valeur]… [--lut film.cube[:intensité]]
  luma xmp <raw|f.xmp|f.json> [--set nom=valeur]… [-o f.xmp]
  luma bench <raw> [--size N] [--runs N]
  luma merge <raw>… [--stack mean|median|max|stars | --pano perspective|cylindrical|spherical]
                    [--no-align] [--no-crop] [--size N] [-o sortie.dng]
  luma film-lut <numérique> <scan> [<numérique> <scan>]… [-o film.cube] [--size 17-65]
                    [--aligned] [--title nom]
  luma presets [dossier]
      Exporte les 100 films en préréglages Lightroom (.xmp), rangés par famille, dans
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

Un fichier XMP posé à côté du RAW (IMG_0001.xmp ou IMG_0001.CR3.xmp, comme ceux de
Lightroom) est lu automatiquement, sauf avec --no-sidecar.";

struct Args {
    positional: Vec<String>,
    output: Option<PathBuf>,
    size: Option<usize>,
    space: OutputSpace,
    quality: u8,
    bits: u8,
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
        space: OutputSpace::Srgb,
        quality: 92,
        bits: 8,
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
                a.space = OutputSpace::parse(&v).ok_or_else(|| format!("espace inconnu « {v} »"))?;
            }
            "--quality" => a.quality = value(&arg)?.parse().map_err(|_| "--quality : 1 à 100")?,
            "--bits" => a.bits = value(&arg)?.parse().map_err(|_| "--bits : 8 ou 16")?,
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
    let size = a.size.map_or(Size::Full, Size::Fit);
    let rendered = photo
        .render(&settings, size, &Cancel::new())
        .map_err(|e| e.to_string())?;
    let out = a.output.clone().unwrap_or_else(|| path.with_extension("jpg"));
    let t2 = Instant::now();
    luma_render::export::write(&out, &rendered.image, a.space, a.quality, a.bits)?;
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

/// `luma presets` : les films intégrés en préréglages Lightroom, un dossier par famille.
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
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    };
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
