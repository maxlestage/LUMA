//! `luma` : le moteur de LÜMA en ligne de commande.
//!
//! ```text
//! luma info <raw>…                  ce que LÜMA lit dans chaque fichier
//! luma develop <raw> [options]      développe et exporte (JPEG, PNG, TIFF)
//! luma export <raw>… [--preset p]… plusieurs photos vers un ou plusieurs préréglages
//! luma export-preset [nom] [opt]    les préréglages d'export : liste, crée, modifie
//! luma xmp <raw|xmp|json> [options] écrit les réglages au format XMP
//! luma bench <raw> [options]        mesure la réactivité (aperçus successifs)
//! luma merge <raw>… [options]       empilement, HDR ou panorama, en DNG
//! luma film-lut <raw> <scan>… [opt]  mesure une vraie pellicule, en LUT 3D (.cube)
//! luma presets [dossier]            exporte les 275 films en préréglages XMP
//! luma cameras [--markdown]         les boîtiers pris en charge (docs/boitiers.md)
//! luma catalog [dossier]… [opt]     la photothèque : ajoute des dossiers, liste par date
//! luma rename <photo>… --name m     renomme par lot (XMP et copies virtuelles suivent)
//! luma geotag <photo>… --gpx t.gpx  place les photos d'après une trace GPX (dans le XMP)
//! luma import [carte] --to dossier  importe une carte (rangement, renommage, 2nd disque)
//! luma print <photo>… -o f.pdf      met en page pour l'imprimante (planche contact…)
//! luma book <photo>… -o livre.pdf   un livre photo en PDF (couverture et titre)
//! luma slideshow <photo>… -o dossier un diaporama autonome (index.html et JPEG)
//! luma gallery <photo>… -o dossier  une galerie web statique, prête à mettre en ligne
//! luma video <in.avi> -o out.avi    un film ou un préréglage sur une vidéo (Motion JPEG)
//! luma colorchecker <raw>           la fidélité des couleurs, sur la photo d'une charte
//! luma match-colors <réf> <autre>   les couleurs d'un boîtier accordées sur un autre
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
                     [--preset nom] [--name modèle] [--resize taille] [--enlarge] [--web]
                     [--max-kb N] [--auto-tone]
                     [--output-sharpen screen|matte|glossy[:low|standard|high]]
                     [--watermark texte] [--watermark-position bottom-right]
                     [--watermark-size 0.5-25] [--watermark-opacity 0-100] [--watermark-dark]
                     [--logo f.png] [--logo-position p] [--logo-size 1-50] [--logo-opacity 0-100]
                     [--author nom] [--copyright texte] [--keep-gps] [--no-exif] [--no-keywords]
      Avec --preset, les options du préréglage d'export (format, qualité, espace, taille,
      dossier, nom) ; les options données les remplacent, --size est alors le grand côté
      de l'image recadrée, et la sortie va dans son dossier (LÜMA/ à côté du RAW par
      défaut). --name donne le nom du fichier par un modèle : {nom} {date} {heure}
      {année} {mois} {jour} {séquence} (ou {séquence:2}…) {boîtier} {marque} {objectif}
      {iso} {note} {libellé} ; un « / » crée un sous‑dossier ({année}/{mois}/{nom}).
      --auto-tone : exposition, contraste, hautes lumières et ombres lus sur
      l'histogramme de la photo (blancs et noirs remis à zéro), sans IA.
      --web : un JPEG allégé pour le web (chrominance 4:2:0 et tables de Huffman
      calculées pour l'image : de 9 à 29 % plus léger ; le préréglage Web le fait).
      --max-kb : le poids maximal d'un JPEG, en kilo‑octets (1 000 octets),
      métadonnées comprises : la meilleure qualité qui tient, --quality au plus.
      --resize : 2048 ou long:2048 (grand côté), short:1080 (petit côté), 1920x1080
      (l'image tient dans le cadre), 12mp (mégapixels), 30x40cm@300 (tirage : le cadre
      en centimètres à 300 ppp, inscrits dans le JPEG et le TIFF) ; jamais d'agrandissement, sauf
      avec --enlarge (Lanczos sans halo, jusqu'à quatre fois chaque côté et 200 Mpx).
      --output-sharpen : l'accentuation de sortie, à la taille finale, pour l'écran ou
      le papier mat ou brillant (écran, mat, brillant ; faible, normale, forte aussi).
      --watermark : un filigrane texte (police de LÜMA), blanc ou noir (--watermark-dark),
      à l'un des neuf emplacements (top-left, top, top-right, left, center, right,
      bottom-left, bottom, bottom-right ; ou haut-gauche… bas-droite), taille en % du
      petit côté (3 par défaut), opacité en % (70 par défaut).
      --logo : un logo en filigrane (PNG, avec sa transparence), mêmes emplacements ;
      grand côté en % du petit côté de l'image (12 par défaut, jamais agrandi au‑delà
      de son fichier), opacité en % (80 par défaut).
      Métadonnées : l'EXIF de la prise de vue (boîtier, objectif, réglages, date) est
      recopié dans les JPEG et PNG (--no-exif : rien), la position GPS est retirée
      (--keep-gps la garde) ; --author et --copyright s'ajoutent à l'EXIF et au XMP ;
      les mots‑clés, la note et le libellé du XMP vont dans les JPEG (--no-keywords).
  luma export <raw>… [--preset nom]… [options de develop, sauf -o]
      Exporte chaque photo vers chacun des préréglages donnés (à défaut, les réglages
      d'export par défaut), en une fois : chaque RAW n'est ouvert qu'une fois, deux
      fichiers du même nom sont numérotés (-2, -3…), {séquence} compte les photos.
  luma export-preset [nom] [--format jpg|png|tif] [--quality 1-100] [--bits 8|16]
                     [--space srgb|p3|rec2020|prophoto] [--size taille|0] [--folder d]
                     [--next-to-original] [--name modèle] [--output-sharpen s|none]
                     [--watermark texte …|--no-watermark] [--logo f.png …|--no-logo]
                     [--author nom] [--copyright texte] [--keep-gps|--remove-gps]
                     [--exif|--no-exif] [--keywords|--no-keywords] [--web|--no-web]
                     [--enlarge|--no-enlarge] [--max-kb N|0] [--delete]
      Sans nom : la liste des préréglages d'export (Web, Pleine définition, Écran P3,
      Impression, puis les vôtres). Avec un nom : crée le préréglage ou le modifie
      (partagé avec l'app) ; --delete le retire.
  luma xmp <raw|f.xmp|f.json> [--set nom=valeur]… [-o f.xmp]
  luma bench <raw> [--size N] [--runs N]
  luma merge <raw>… [--stack mean|median|max|stars|hdr|focus | --pano perspective|cylindrical|spherical]
                    [--no-align] [--no-crop] [--warp] [--size N] [-o sortie.dng]
      --warp (panorama) : les bords irréguliers sont étirés pour remplir le rectangle,
      au lieu d'être rognés.
  luma film-lut <numérique> <scan> [<numérique> <scan>]… [-o film.cube] [--size 17-65]
                    [--aligned] [--title nom]
  luma watch <dossier> [--catalog f.lumacat] [--interval secondes]
      Surveille un dossier (prise de vue en connexion, carte copiée) : chaque photo
      arrivée, une fois complète, entre dans la photothèque et s'affiche (boîtier,
      objectif, réglages). Relevé toutes les secondes par défaut ; Ctrl+C pour arrêter.
  luma rename <photo>… --name modèle [--dry-run] [--catalog f.lumacat]
      Renomme les photos d'après un modèle, dans l'ordre de prise de vue : les repères de
      --name de l'export ({date}_{séquence}, {boîtier}-{nom}…). Le RAW et le JPEG d'une
      même prise gardent un même nom ; le XMP et les copies virtuelles suivent ; un nom
      déjà pris reçoit -2. Au moindre échec, tout revient comme avant. --dry-run montre
      les nouveaux noms sans rien renommer. La photothèque suit.
  luma print <photo>… -o sortie.pdf [--paper A4|A3|A3+|A5|Letter|10x15|13x18|20x25|20x30|LxH]
                     [--landscape] [--margin mm] [--grid 4x5] [--gap mm] [--dpi 300]
                     [--captions] [--fill] [--profile imprimante.icc]
                     [--intent perceptual|relative|saturation|absolute]
                     [--crop-marks] [--text \"© Nom 2026\"]
                     [--output-sharpen matte|glossy[:low|standard|high]]
      Met les photos en page pour l'imprimante, dans un PDF à la taille exacte du papier :
      une photo par page (entière, ou remplissant la page avec --fill), ou une planche
      contact avec --grid (colonnes × lignes, le nom de chaque photo dessous avec
      --captions). Marges de 10 mm et 300 points par pouce par défaut. --profile
      convertit chaque page vers le profil RVB de l'imprimante (colorimétrique relative
      par défaut, noir compensé) : imprimez alors sans gestion des couleurs du pilote.
      --crop-marks : des repères de coupe aux coins de chaque photo ; --text : une ligne
      de texte au bas de chaque page (un titre, un nom, des droits).
  luma book <photo>… -o livre.pdf [--title titre] [--subtitle texte]
                    [--format 20x20|30x30|A4|21x28|A3|LxH] [--landscape] [--margin mm]
                    [--gap mm] [--dpi 300] [--captions] [--profile imprimante.icc]
                    [--opening texte] [--closing texte] [--pages 1,2,4]
      Un livre photo : la couverture (la première photo, le titre et le sous‑titre
      dessous), puis des pages d'une, deux ou quatre photos selon leur orientation (deux
      paysages l'un sous l'autre, deux portraits côte à côte). 20 × 20 cm, marges de
      15 mm et 300 points par pouce par défaut. --opening : une page de texte après la
      couverture (une dédicace) ; --closing : une dernière page de texte ; « \\n » dans
      le texte passe à la ligne. --pages : le nombre de photos de chaque page après la
      couverture (1, 2 ou 4), répété jusqu'à la fin ; sans : choisi d'après les photos.
  luma slideshow <photo>… -o dossier [--title titre] [--seconds 5] [--fade 1]
                         [--size 2048] [--quality 85] [--captions]
                         [--transition fade|slide|drift] [--music fichier]
      Un diaporama qui se suffit à lui‑même : le dossier reçoit index.html et les photos
      (JPEG sRGB, grand côté de 2048 px par défaut), à ouvrir dans un navigateur ou à
      mettre en ligne. Flèches ou glissement du doigt, espace pour la pause, F pour le
      plein écran. --transition : fondu (fade, par défaut), glissement (slide), ou fondu
      avec un lent zoom (drift). --music : une musique (mp3, m4a, aac, ogg, wav, flac)
      copiée dans le dossier, jouée en boucle ; M la coupe.
  luma gallery <photo>… -o dossier [--title titre] [--description texte] [--size 2048]
                       [--thumb 480] [--quality 85] [--no-captions]
      Une galerie web statique : index.html (la grille des vignettes, chaque photo en
      grand d'un clic, flèches, glissement du doigt, Échap), photos/ et vignettes/ en
      JPEG allégés pour le web. À mettre en ligne tel quel. La légende de chaque photo
      est son titre (informations IPTC), sinon son nom.
  luma video <vidéo.avi> -o sortie.avi [--film nom[:intensité]] [--xmp f.xmp]
             [--set nom=valeur]… [--lut film.cube[:intensité]] [--quality 1-100]
      Applique un film (l'un des 100 de LÜMA, par son nom : « Portra 400 »), un
      préréglage XMP ou des réglages à chaque image d'une vidéo AVI en Motion JPEG
      (celle de nombreux appareils photo), puis l'enregistre au même format : même
      taille, même rythme, le son recopié tel quel. Pour une vidéo courte (1 Go au plus).
  luma geotag <photo|dossier>… --gpx trace.gpx [--offset +02:00] [--max-gap 3600]
              [--replace] [--dry-run]
      Place les photos d'après une trace GPX (téléphone, montre, GPS) : chacune prend la
      position de la trace à l'instant de sa prise de vue, entre les deux points qui
      l'encadrent, écrite dans son XMP (le fichier n'est pas touché ; la photothèque et
      la carte la lisent). --offset : l'écart de l'horloge du boîtier à l'heure
      universelle (+02:00 pour un boîtier à l'heure d'été de Paris ; 0 par défaut ;
      auto : l'écart qui fait tomber le plus de photos dans la trace).
      --max-gap : le plus long trou de la trace à franchir, en secondes (une heure par
      défaut). Une photo qui a déjà une position la garde, sauf avec --replace ;
      --dry-run montre ce qui serait fait.
  luma import [carte|dossier] --to dossier [--folders \"{année}/{année}-{mois}-{jour}\"]
              [--name modèle] [--backup second-disque] [--dry-run] [--catalog f.lumacat]
              [--days 2026-10-26,2026-10-27] [--skip photo]… [--eject]
      Importe les photos d'une carte mémoire (sans carte donnée : la carte montée, celle
      qui a un dossier DCIM) ou d'un dossier : copiées dans --to, rangées par date de
      prise de vue (--folders, vide : tout au même endroit), renommées avec --name (les
      repères de l'export : {date}_{séquence}, {boîtier}-{nom}…), et copiées aussi sur
      --backup. Chaque copie est relue et comparée à l'original ; le RAW, son JPEG et
      son XMP gardent un même nom ; une photo déjà importée est sautée ; la carte n'est
      jamais modifiée. Les photos importées entrent dans la photothèque. Les jours de
      prise de vue de la carte s'affichent ; --days n'importe que ceux‑là. --skip laisse
      une photo sur la carte (avec les autres fichiers de sa prise : son JPEG, son RAW).
      --eject éjecte la carte une fois tout copié et vérifié (Mac, Linux).
  luma icc <profil.icc>…
      Lit un profil ICC (versions 2 et 4 ; matrice et courbes, ou tables lut8, lut16,
      A→B et B→A) : sa classe, son espace, ses intentions de rendu, le blanc du support,
      et ce que donnent le blanc, le noir et les primaires du périphérique.
  luma catalog [dossier]… [--flat] [--catalog f.lumacat] [--list] [--search \"…\"] [--keywords]
               [--smart nom \"règle\"] [--add nom photo]… [--remove-collection nom]
               [--collections] [--collection nom] [--backup] [--backups] [--restore f]
               [--missing] [--relocate ancien nouveau] [--duplicates]
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
      Entretien : --backup copie le catalogue dans « Sauvegardes » à côté de lui (les dix
      dernières sont gardées ; l'app en fait une par semaine), --backups les liste,
      --restore remet l'une d'elles (le catalogue remplacé est sauvegardé d'abord).
      --missing liste les photos dont le fichier n'est plus là ; --relocate retrouve
      celles d'un dossier déplacé à sa nouvelle place, collections comprises.
      --duplicates liste les photos en double : même boîtier, même instant, fichiers de
      même taille et de même type (une carte importée deux fois, une copie ailleurs) ;
      rien n'est effacé.
  luma cameras [--markdown] [--corpus dossier]
      Les boîtiers pris en charge ; --markdown écrit la matrice publiée (docs/boitiers.md),
      --corpus marque ceux dont un vrai RAW est dans le dossier.
  luma colorchecker <raw> [--xmp f.xmp] [-o profil.xmp] [--name nom]
      Mesure la fidélité des couleurs d'un boîtier : photographier une charte ColorChecker
      (24 plages), la recadrer au plus juste dans LÜMA (le XMP garde le recadrage), puis
      lancer la commande : ΔE 2000 de chaque plage (moins de 2 : invisible ; moins de 5 :
      bon). Avec -o, écrit le profil du boîtier mesuré sur la charte : un préréglage XMP
      (mélangeur de couches, le blanc reste neutre) à importer dans les Préréglages et à
      poser sur les photos de ce boîtier.
  luma match-colors <référence> <autre> [-o accord.xmp] [--name nom]
      Accorde les couleurs de deux boîtiers : la même scène photographiée par l'un
      (référence) et par l'autre, même cadrée un peu autrement. Écart de couleur avant et
      après ; avec -o, écrit la correction du second boîtier en préréglage XMP (mélangeur
      de couches, le blanc reste neutre) à poser sur ses photos.
  luma licence-keys
      Une nouvelle paire de clés pour LÜMA Premium : la privée va sur le serveur
      (LUMA_LICENCE_SECRET, à garder secrète), la publique dans l'app (LUMA_LICENCE_PUBLIC).
  luma presets [dossier]
      Exporte les 275 films en préréglages XMP (.xmp), rangés par famille, dans
      « dossier/Préréglages LÜMA » (dossier courant par défaut).
      Mesure une pellicule : la même scène en numérique (RAW, développé avec les réglages
      neutres de LÜMA) et sur film (scan JPEG, PNG ou TIFF). Les images sont alignées
      automatiquement (--aligned si elles se superposent déjà). La LUT obtenue, posée
      sur n'importe quelle photo, lui donne le rendu de cette pellicule.

RÉGLAGES (--set)
  exposure contrast highlights highlights.reconstruction=on|off shadows whites blacks
  texture clarity dehaze vibrance
  saturation temperature tint curve.highlights|lights|darks|shadows
  hsl.hue|saturation|luminance.<red|orange|yellow|green|aqua|blue|purple|magenta>
  grading.<shadows|midtones|highlights|global>.<hue|saturation|luminance>
  grading.blending grading.balance sharpen sharpen.radius sharpen.detail sharpen.masking
  sharpen.deconvolution=on|off noise.luminance noise.wavelets=on|off noise.color vignette vignette.midpoint vignette.feather
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
    presets: Vec<String>,
    name: Option<String>,
    resize: Option<export::Resize>,
    output_sharpen: Option<export::OutputSharpen>,
    watermark: WatermarkArgs,
    logo: LogoArgs,
    metadata: MetaArgs,
    xmp: Option<PathBuf>,
    settings: Option<PathBuf>,
    sidecar: bool,
    sets: Vec<String>,
    runs: usize,
    lut: Option<String>,
    /// `--web` / `--no-web` : le JPEG allégé pour le web.
    web: Option<bool>,
    /// `--enlarge` / `--no-enlarge` : agrandir au‑delà de la pleine définition.
    enlarge: Option<bool>,
    /// `--max-kb N` : le poids maximal d'un JPEG, en kilo‑octets (0 : aucun).
    max_kb: Option<u32>,
    /// `--auto-tone` : exposition, contraste, hautes lumières et ombres lus sur la photo.
    auto_tone: bool,
}

/// L'erreur d'un `--max-kb` illisible.
const MAX_KB: &str = "--max-kb : un poids en kilo-octets (0 : aucun)";

fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        positional: Vec::new(),
        output: None,
        size: None,
        space: None,
        quality: None,
        bits: None,
        presets: Vec::new(),
        name: None,
        resize: None,
        output_sharpen: None,
        watermark: WatermarkArgs::default(),
        logo: LogoArgs::default(),
        metadata: MetaArgs::default(),
        xmp: None,
        settings: None,
        sidecar: true,
        sets: Vec::new(),
        runs: 20,
        lut: None,
        web: None,
        enlarge: None,
        max_kb: None,
        auto_tone: false,
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
            "--preset" => a.presets.push(value(&arg)?),
            "--web" => a.web = Some(true),
            "--no-web" => a.web = Some(false),
            "--enlarge" => a.enlarge = Some(true),
            "--no-enlarge" => a.enlarge = Some(false),
            "--max-kb" => a.max_kb = Some(value(&arg)?.parse().map_err(|_| MAX_KB)?),
            "--auto-tone" => a.auto_tone = true,
            "--name" => a.name = Some(value(&arg)?),
            s if s.starts_with("--watermark") => {
                let v = if s == "--watermark-dark" {
                    String::new()
                } else {
                    value(&arg)?
                };
                a.watermark.read(s, v)?;
            }
            s if s.starts_with("--logo") => {
                let v = value(&arg)?;
                a.logo.read(s, v)?;
            }
            s if MetaArgs::FLAGS.contains(&s) => a.metadata.flag(s),
            s @ ("--author" | "--copyright") => {
                let v = value(&arg)?;
                a.metadata.text(s, v);
            }
            "--output-sharpen" => {
                let v = value(&arg)?;
                a.output_sharpen = Some(export::OutputSharpen::parse(&v).ok_or_else(|| sharpen_error(&v))?);
            }
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
    // Sauvegardes et photos manquantes.
    let (mut save_copy, mut list_backups, mut missing, mut duplicates) = (false, false, false, false);
    let mut restore = None;
    let mut relocate = None;
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
            "--backup" => save_copy = true,
            "--backups" => list_backups = true,
            "--restore" => restore = Some(PathBuf::from(it.next().ok_or("--restore attend une sauvegarde")?)),
            "--missing" => missing = true,
            "--duplicates" => duplicates = true,
            "--relocate" => {
                let from = it.next().ok_or("--relocate attend l'ancien dossier")?;
                let to = it.next().ok_or("--relocate attend le nouveau dossier")?;
                relocate = Some((PathBuf::from(from), PathBuf::from(to)));
            }
            s if s.starts_with('-') => return Err(format!("option inconnue « {s} »")),
            _ => folders.push(PathBuf::from(arg)),
        }
    }
    let file = file.ok_or("pas de dossier personnel pour le catalogue : --catalog f.lumacat")?;
    use luma_library::backup;
    if let Some(from) = &restore {
        backup::restore(from, &file).map_err(|e| format!("{} : {e}", from.display()))?;
        println!("{} remis à la place du catalogue", from.display());
    }
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
    let mut moved = false;
    if let Some((from, to)) = &relocate {
        let n = cat.relocate(from, to);
        println!("{n} photos retrouvées dans {}", to.display());
        moved = n > 0;
    }
    if missing {
        let gone = cat.missing();
        println!("{} photos manquantes", gone.len());
        for e in gone {
            println!("  {}", e.path.display());
        }
    }
    if duplicates {
        let groups = cat.duplicates();
        let extra: u64 = groups.iter().map(|g| g[0].size * (g.len() as u64 - 1)).sum();
        println!(
            "{} photos en double ({:.1} Mo de copies en trop)",
            groups.len(),
            extra as f64 / 1e6
        );
        for g in &groups {
            println!("  {} · {}", g[0].date.replace('T', " "), g[0].camera());
            for e in g {
                println!("    {}", e.path.display());
            }
        }
    }
    if !folders.is_empty() || !edits.is_empty() || moved {
        cat.save(&file).map_err(|e| format!("{} : {e}", file.display()))?;
    }
    if save_copy {
        let to = backup::backup(&file).map_err(|e| format!("{} : {e}", file.display()))?;
        println!("sauvegarde : {}", to.display());
    }
    if list_backups {
        for b in backup::list(&file) {
            println!("{}", b.display());
        }
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

/// Les tons automatiques posés sur les réglages (`--auto-tone`).
fn auto_tone(photo: &Photo, s: &mut Settings) {
    let t = photo.auto_tones(s);
    s.exposure = t.exposure;
    s.contrast = t.contrast;
    s.highlights = t.highlights;
    s.shadows = t.shadows;
    s.whites = 0.0;
    s.blacks = 0.0;
}

fn develop(a: &Args) -> Result<(), String> {
    let [path] = a.positional.as_slice() else {
        return Err("develop : un seul fichier RAW".into());
    };
    let path = Path::new(path);
    let mut settings = load_settings(a, Some(path))?;
    let t = Instant::now();
    let photo = Photo::open(path).map_err(|e| e.to_string())?;
    let opened = t.elapsed();
    if a.auto_tone {
        auto_tone(&photo, &mut settings);
    }
    if a.presets.len() > 1 {
        return Err("develop : un seul préréglage (luma export pour en utiliser plusieurs)".into());
    }
    if !a.presets.is_empty()
        || a.name.is_some()
        || a.resize.is_some()
        || a.enlarge.is_some()
        || a.max_kb.is_some()
        || a.output_sharpen.is_some()
        || !a.watermark.is_empty()
        || !a.logo.is_empty()
        || !a.metadata.is_empty()
    {
        let presets = if a.presets.is_empty() {
            export::Presets::default()
        } else {
            load_presets()?.1
        };
        let mut o = export_options(a, a.presets.first().map(String::as_str), &presets)?;
        if let Some(f) = a
            .output
            .as_deref()
            .and_then(|p| p.extension()?.to_str())
            .and_then(Format::parse)
        {
            o.format = f;
        }
        let meta = sidecar_meta(a, path);
        let fields = export::NameFields::of_photo(path, &photo, &meta, 1);
        let out = a.output.clone().unwrap_or_else(|| {
            let mut taken = std::collections::HashSet::new();
            export::reserve_sources(&mut taken, [path]);
            export::unique(export::output_path(path, &o, &fields), &mut taken)
        });
        let t2 = Instant::now();
        export::develop(&photo, &settings, &out, &o, &meta)?;
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
    // Les métadonnées d'un export ordinaire : EXIF recopié, GPS retiré.
    let raw = photo.raw();
    let embedded = export::Embedded::new(
        &export::PhotoMetadata {
            make: raw.make.clone(),
            model: raw.model.clone(),
            exif: raw.exif.clone(),
            meta: sidecar_meta(a, path),
        },
        &export::MetadataOptions::default(),
        rendered.image.width,
        rendered.image.height,
    );
    export::write_file(
        &out,
        &rendered.image,
        a.space.unwrap_or_default(),
        a.quality.unwrap_or(92),
        a.bits.unwrap_or(8),
        &embedded,
        a.web.unwrap_or(false),
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

/// Les options `--watermark…` : le texte, puis ce qui le règle ; appliquées à un
/// filigrane existant (celui d'un préréglage) ou à un nouveau.
#[derive(Default)]
struct WatermarkArgs {
    text: Option<String>,
    position: Option<export::Position>,
    size: Option<f32>,
    opacity: Option<f32>,
    dark: bool,
}

impl WatermarkArgs {
    fn read(&mut self, option: &str, v: String) -> Result<(), String> {
        let number = |v: &str, min: f32, max: f32| {
            v.trim()
                .replace(',', ".")
                .parse::<f32>()
                .ok()
                .filter(|n| (min..=max).contains(n))
                .ok_or_else(|| format!("{option} : un nombre de {min} à {max}"))
        };
        match option {
            "--watermark" => self.text = Some(v),
            "--watermark-position" => {
                self.position = Some(export::Position::parse(&v).ok_or_else(|| {
                    let all: Vec<&str> = export::Position::ALL.iter().map(|p| p.id()).collect();
                    format!("emplacement inconnu « {v} » ({})", all.join(", "))
                })?);
            }
            "--watermark-size" => self.size = Some(number(&v, 0.5, 25.0)?),
            "--watermark-opacity" => self.opacity = Some(number(&v, 0.0, 100.0)? / 100.0),
            "--watermark-dark" => self.dark = true,
            other => return Err(format!("option inconnue « {other} »")),
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.text.is_none() && self.position.is_none() && self.size.is_none() && self.opacity.is_none() && !self.dark
    }

    fn apply(&self, w: &mut Option<export::Watermark>) {
        if let Some(t) = &self.text {
            *w = if t.trim().is_empty() {
                None
            } else {
                Some(match w.take() {
                    Some(old) => export::Watermark { text: t.clone(), ..old },
                    None => export::Watermark::new(t),
                })
            };
        }
        if let Some(w) = w {
            w.position = self.position.unwrap_or(w.position);
            w.size = self.size.unwrap_or(w.size);
            w.opacity = self.opacity.unwrap_or(w.opacity);
            w.dark |= self.dark;
        }
    }
}

/// Les options des métadonnées : EXIF, GPS, mots‑clés, auteur, copyright.
#[derive(Default)]
struct MetaArgs {
    copy_exif: Option<bool>,
    keep_gps: Option<bool>,
    keywords: Option<bool>,
    author: Option<String>,
    copyright: Option<String>,
}

impl MetaArgs {
    const FLAGS: [&'static str; 6] = [
        "--exif",
        "--no-exif",
        "--keep-gps",
        "--remove-gps",
        "--keywords",
        "--no-keywords",
    ];

    fn flag(&mut self, f: &str) {
        match f {
            "--exif" => self.copy_exif = Some(true),
            "--no-exif" => self.copy_exif = Some(false),
            "--keep-gps" => self.keep_gps = Some(true),
            "--remove-gps" => self.keep_gps = Some(false),
            "--keywords" => self.keywords = Some(true),
            _ => self.keywords = Some(false),
        }
    }

    /// Un texte vide efface l'auteur ou le copyright.
    fn text(&mut self, option: &str, v: String) {
        if option == "--author" {
            self.author = Some(v);
        } else {
            self.copyright = Some(v);
        }
    }

    fn is_empty(&self) -> bool {
        self.copy_exif.is_none()
            && self.keep_gps.is_none()
            && self.keywords.is_none()
            && self.author.is_none()
            && self.copyright.is_none()
    }

    fn apply(&self, m: &mut export::MetadataOptions) {
        m.copy_exif = self.copy_exif.unwrap_or(m.copy_exif);
        m.keep_gps = self.keep_gps.unwrap_or(m.keep_gps);
        m.keywords = self.keywords.unwrap_or(m.keywords);
        let text = |v: &Option<String>, old: &mut Option<String>| {
            if let Some(v) = v {
                *old = Some(v.trim().to_owned()).filter(|v| !v.is_empty());
            }
        };
        text(&self.author, &mut m.author);
        text(&self.copyright, &mut m.copyright);
    }
}

/// Les options `--logo…` : le fichier, puis ce qui le règle.
#[derive(Default)]
struct LogoArgs {
    path: Option<PathBuf>,
    position: Option<export::Position>,
    size: Option<f32>,
    opacity: Option<f32>,
}

impl LogoArgs {
    fn read(&mut self, option: &str, v: String) -> Result<(), String> {
        let number = |v: &str, min: f32, max: f32| {
            v.trim()
                .replace(',', ".")
                .parse::<f32>()
                .ok()
                .filter(|n| (min..=max).contains(n))
                .ok_or_else(|| format!("{option} : un nombre de {min} à {max}"))
        };
        match option {
            "--logo" => {
                let p = PathBuf::from(&v);
                if !p.is_file() {
                    return Err(format!("--logo : pas de fichier {}", p.display()));
                }
                // Un chemin complet : un préréglage enregistré le retrouve d'ailleurs.
                self.path = Some(std::fs::canonicalize(&p).unwrap_or(p));
            }
            "--logo-position" => {
                self.position = Some(export::Position::parse(&v).ok_or_else(|| {
                    let all: Vec<&str> = export::Position::ALL.iter().map(|p| p.id()).collect();
                    format!("emplacement inconnu « {v} » ({})", all.join(", "))
                })?);
            }
            "--logo-size" => self.size = Some(number(&v, 1.0, 50.0)?),
            "--logo-opacity" => self.opacity = Some(number(&v, 0.0, 100.0)? / 100.0),
            other => return Err(format!("option inconnue « {other} »")),
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.path.is_none() && self.position.is_none() && self.size.is_none() && self.opacity.is_none()
    }

    fn apply(&self, l: &mut Option<export::Logo>) {
        if let Some(p) = &self.path {
            *l = Some(match l.take() {
                Some(old) => export::Logo { path: p.clone(), ..old },
                None => export::Logo::new(p),
            });
        }
        if let Some(l) = l {
            l.position = self.position.unwrap_or(l.position);
            l.size = self.size.unwrap_or(l.size);
            l.opacity = self.opacity.unwrap_or(l.opacity);
        }
    }
}

fn sharpen_error(v: &str) -> String {
    format!("accentuation inconnue « {v} » (screen, matte, glossy, suivis de :low, :standard ou :high)")
}

fn resize_error(v: &str) -> String {
    format!("taille inconnue « {v} » (2048, short:1080, 1920x1080, 12mp)")
}

/// Les options d'un export : celles du préréglage (à défaut, les options par défaut),
/// que les options données remplacent.
fn export_options(a: &Args, preset: Option<&str>, presets: &export::Presets) -> Result<export::Options, String> {
    let mut o = match preset {
        Some(name) => {
            presets
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
    o.sharpen = a.output_sharpen.or(o.sharpen);
    a.watermark.apply(&mut o.watermark);
    a.logo.apply(&mut o.logo);
    a.metadata.apply(&mut o.metadata);
    o.quality = a.quality.unwrap_or(o.quality);
    o.bits = a.bits.unwrap_or(o.bits);
    o.web = a.web.unwrap_or(o.web);
    o.enlarge = a.enlarge.unwrap_or(o.enlarge);
    if let Some(k) = a.max_kb {
        o.max_kb = (k > 0).then_some(k);
    }
    o.resize = a
        .resize
        .or(a.size.filter(|&n| n > 0).map(export::Resize::LongEdge))
        .or(o.resize);
    Ok(o)
}

/// `luma export` : des photos vers un ou plusieurs préréglages à la fois. Chaque photo est
/// ouverte une fois, puis développée pour chaque préréglage ; deux fichiers du même nom
/// sont numérotés plutôt qu'écrasés. Une photo illisible n'arrête pas les autres.
fn export_many(a: &Args) -> Result<(), String> {
    if a.positional.is_empty() {
        return Err("export : au moins une photo".into());
    }
    if a.output.is_some() {
        return Err("export : pas de -o (chaque préréglage a son dossier ; --folder… avec luma export-preset)".into());
    }
    let presets = load_presets()?.1;
    let targets: Vec<export::Options> = if a.presets.is_empty() {
        vec![export_options(a, None, &presets)?]
    } else {
        a.presets
            .iter()
            .map(|p| export_options(a, Some(p), &presets))
            .collect::<Result<_, _>>()?
    };
    let start = Instant::now();
    let mut taken = std::collections::HashSet::new();
    export::reserve_sources(&mut taken, a.positional.iter().map(Path::new));
    let (mut written, mut failed) = (0, 0);
    for (k, path) in a.positional.iter().enumerate() {
        let path = Path::new(path);
        let result = (|| -> Result<(), String> {
            let mut settings = load_settings(a, Some(path))?;
            let photo = Photo::open(path).map_err(|e| e.to_string())?;
            if a.auto_tone {
                auto_tone(&photo, &mut settings);
            }
            let meta = sidecar_meta(a, path);
            let fields = export::NameFields::of_photo(path, &photo, &meta, k + 1);
            for o in &targets {
                let out = export::unique(export::output_path(path, o, &fields), &mut taken);
                export::develop(&photo, &settings, &out, o, &meta)?;
                println!("{} → {}", path.display(), out.display());
                written += 1;
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("erreur : {} : {e}", path.display());
            failed += 1;
        }
    }
    println!(
        "{written} fichier(s) en {:.1} s, {} photo(s), {} préréglage(s)",
        start.elapsed().as_secs_f64(),
        a.positional.len(),
        targets.len()
    );
    if failed > 0 {
        return Err(format!("{failed} photo(s) non exportée(s)"));
    }
    Ok(())
}

/// La note, le libellé et les mots‑clés du XMP posé à côté du RAW (sauf --no-sidecar).
fn sidecar_meta(a: &Args, path: &Path) -> xmp::Meta {
    a.sidecar
        .then(|| sidecar(path))
        .flatten()
        .and_then(|x| std::fs::read_to_string(x).ok())
        .map(|t| xmp::read_meta(&t))
        .unwrap_or_default()
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
            "--web" => o.web = true,
            "--no-web" => o.web = false,
            "--enlarge" => o.enlarge = true,
            "--no-enlarge" => o.enlarge = false,
            "--max-kb" => {
                let k: u32 = value()?.parse().map_err(|_| MAX_KB)?;
                o.max_kb = (k > 0).then_some(k);
            }
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
            "--no-watermark" => o.watermark = None,
            "--no-logo" => o.logo = None,
            s if MetaArgs::FLAGS.contains(&s) => {
                let mut m = MetaArgs::default();
                m.flag(s);
                m.apply(&mut o.metadata);
            }
            s @ ("--author" | "--copyright") => {
                let mut m = MetaArgs::default();
                m.text(s, value()?);
                m.apply(&mut o.metadata);
            }
            s if s.starts_with("--logo") => {
                let mut l = LogoArgs::default();
                l.read(s, value()?)?;
                l.apply(&mut o.logo);
            }
            s if s.starts_with("--watermark") => {
                let v = if s == "--watermark-dark" {
                    String::new()
                } else {
                    value()?
                };
                let mut w = WatermarkArgs::default();
                w.read(s, v)?;
                w.apply(&mut o.watermark);
            }
            "--output-sharpen" => {
                let v = value()?;
                o.sharpen = if v.trim() == "0" || v.trim() == "none" || v.trim() == "aucune" {
                    None
                } else {
                    Some(export::OutputSharpen::parse(&v).ok_or_else(|| sharpen_error(&v))?)
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

/// `luma merge` : empilement (bruit, médiane, filé, étoiles, mise au point), HDR ou panorama, en DNG.
fn merge(args: Vec<String>) -> Result<(), String> {
    use luma_merge::{Projection, StackMode};
    let mut paths = Vec::new();
    let mut stack = Some(StackMode::Mean);
    let mut pano = Projection::Cylindrical;
    let (mut align, mut crop, mut size, mut out) = (true, true, Size::Full, None);
    let mut warp = false;
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
                    "hdr" => StackMode::Hdr,
                    "focus" | "mise-au-point" => StackMode::Focus,
                    v => {
                        return Err(format!(
                            "empilement inconnu « {v} » (mean, median, max, stars, hdr, focus)"
                        ));
                    }
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
            "--warp" => warp = true,
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
    let (mut merged, suffix) = match stack {
        Some(mode) => (
            luma_merge::stack(&frames, mode, align, &progress, &cancel).map_err(|e| e.to_string())?,
            mode.name(),
        ),
        None => (
            luma_merge::panorama(&frames, pano, &progress, &cancel).map_err(|e| e.to_string())?,
            "Pano",
        ),
    };
    if warp && stack.is_none() {
        merged.warp_edges();
    }
    eprintln!();
    let template = luma_raw::RawPhoto::open(&paths[paths.len() / 2]).map_err(|e| e.to_string())?;
    let out = out.unwrap_or_else(|| luma_merge::output_path(&paths[0], &suffix.replace('é', "e").replace(' ', "-")));
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
    if merged.exposure > 0.0 {
        println!(
            "  {:.1} IL de lumières gardées au-delà de la pose de référence (exposition de base)",
            merged.exposure
        );
    }
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
    // La couleur seule : pas de look, pas de courbe (l'étape d'entrée et la balance) ; le
    // mélangeur reste, pour vérifier un profil déjà posé (ou l'affiner).
    let keep = (
        s.white_balance,
        s.crop,
        s.transform.clone(),
        s.lens.clone(),
        s.channel_mixer,
    );
    s = Settings::default();
    (s.white_balance, s.crop, s.transform, s.lens, s.channel_mixer) = keep;
    let photo = Photo::open(raw).map_err(|e| e.to_string())?;
    let mut t = luma_core::Timings::default();
    let base = photo
        .base(&s, Size::Fit(1600), &Cancel::new(), &mut t)
        .map_err(|e| e.to_string())?;
    let profile = luma_render::chart::fit_profile(&base.image);
    let de = profile.before;
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
    let after = profile.after.iter().sum::<f32>() / 24.0;
    let worst_after = profile.after.iter().copied().fold(0.0, f32::max);
    println!("avec le profil du boîtier : moyenne {after:.2}, maximum {worst_after:.2}");
    if let Some(out) = &a.output {
        let name = a
            .name
            .clone()
            .unwrap_or_else(|| format!("Profil {} {}", photo.raw().make, photo.raw().model));
        // Mesuré par‑dessus le mélangeur déjà posé : le profil est les deux à la suite.
        let total = luma_color::mul(&profile.matrix, &s.channel_mixer.matrix());
        let settings = Settings {
            channel_mixer: luma_project::ChannelMixer {
                red: total[0].map(|v| v * 100.0),
                green: total[1].map(|v| v * 100.0),
                blue: total[2].map(|v| v * 100.0),
            },
            ..Settings::default()
        }
        .clamped();
        let text = xmp::write_preset(&name, "Profils de boîtier", &settings);
        std::fs::write(out, text).map_err(|e| format!("{} : {e}", out.display()))?;
        println!("profil écrit : {} (« {name} »)", out.display());
    }
    Ok(())
}

/// `luma match-colors` : les couleurs d'un second boîtier accordées sur celles d'un
/// premier, d'après la même scène.
fn match_colors(a: &Args) -> Result<(), String> {
    let [reference, other] = a.positional.as_slice() else {
        return Err("deux photos de la même scène : la référence, puis l'autre boîtier".into());
    };
    let develop = |path: &str| -> Result<(std::sync::Arc<Photo>, luma_core::RgbImage), String> {
        let photo = std::sync::Arc::new(Photo::open(Path::new(path)).map_err(|e| e.to_string())?);
        // Développées sans look (balance telle quelle), comme pour la charte.
        let mut t = luma_core::Timings::default();
        let base = photo
            .base(&Settings::default(), Size::Fit(1600), &Cancel::new(), &mut t)
            .map_err(|e| e.to_string())?;
        Ok((photo.clone(), (*base.image).clone()))
    };
    let (ref_photo, ref_img) = develop(reference)?;
    let (photo, img) = develop(other)?;
    let m = luma_merge::matching::match_colors(&ref_img, &img).map_err(|e| e.to_string())?;
    println!(
        "{} {} accordé sur {} {} : écart de couleur {:.2} → {:.2} (ΔE 2000, {} points)",
        photo.raw().make,
        photo.raw().model,
        ref_photo.raw().make,
        ref_photo.raw().model,
        m.before,
        m.after,
        m.samples
    );
    if let Some(out) = &a.output {
        let name = a.name.clone().unwrap_or_else(|| {
            format!(
                "Accord {} {} sur {} {}",
                photo.raw().make,
                photo.raw().model,
                ref_photo.raw().make,
                ref_photo.raw().model
            )
        });
        let row = |r: usize| m.matrix[r].map(|v| v * 100.0);
        let settings = Settings {
            channel_mixer: luma_project::ChannelMixer {
                red: row(0),
                green: row(1),
                blue: row(2),
            },
            ..Settings::default()
        }
        .clamped();
        let text = xmp::write_preset(&name, "Profils de boîtier", &settings);
        std::fs::write(out, text).map_err(|e| format!("{} : {e}", out.display()))?;
        println!("accord écrit : {} (« {name} »)", out.display());
    }
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

/// Surveille un dossier : chaque photo arrivée entre dans la photothèque.
fn watch(args: Vec<String>) -> Result<(), String> {
    let mut dir = None;
    let mut file = None;
    let mut interval = 1.0f64;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--catalog" => file = Some(PathBuf::from(it.next().ok_or("--catalog : fichier attendu")?)),
            "--interval" => {
                interval = it
                    .next()
                    .and_then(|v| v.replace(',', ".").parse().ok())
                    .filter(|v: &f64| *v >= 0.1)
                    .ok_or("--interval : nombre de secondes attendu (0,1 au moins)")?
            }
            _ => dir = Some(PathBuf::from(a)),
        }
    }
    let dir = dir.ok_or("il faut un dossier à surveiller")?;
    if !dir.is_dir() {
        return Err(format!("{} n'est pas un dossier", dir.display()));
    }
    let file = file
        .or_else(luma_library::default_path)
        .ok_or("pas de photothèque (--catalog)")?;
    let mut catalog = luma_library::Catalog::load(&file);
    let mut w = luma_library::watch::Watch::new(&dir);
    eprintln!("surveillance de {} (Ctrl+C pour arrêter)", dir.display());
    loop {
        let new = w.poll();
        if !new.is_empty() {
            catalog.add_folder(&dir, false);
            catalog.save(&file).map_err(|e| e.to_string())?;
            for p in new {
                let name = p
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let about = catalog.get(&p).map_or_else(String::new, |e| {
                    let mut parts = vec![format!("{} {}", e.make, e.model).trim().to_owned()];
                    parts.push(e.lens.clone());
                    if let Some(iso) = e.iso {
                        parts.push(format!("ISO {iso}"));
                    }
                    if let Some(f) = e.aperture {
                        parts.push(format!("f/{f:.1}"));
                    }
                    parts.retain(|s| !s.is_empty());
                    parts.join(" · ")
                });
                println!("{name}\t{about}");
            }
        }
        std::thread::sleep(Duration::from_secs_f64(interval));
    }
}

/// Renomme des photos par lot.
fn rename(args: Vec<String>) -> Result<(), String> {
    let mut template = None;
    let mut dry = false;
    let mut file = None;
    let mut photos = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--name" => template = Some(it.next().ok_or("--name : modèle attendu")?),
            "--dry-run" | "-n" => dry = true,
            "--catalog" => file = Some(PathBuf::from(it.next().ok_or("--catalog : fichier attendu")?)),
            _ => photos.push(PathBuf::from(a)),
        }
    }
    let template = template.ok_or("il faut un modèle (--name \"{date}_{séquence}\")")?;
    if photos.is_empty() {
        return Err("aucune photo à renommer".into());
    }
    // Dans l'ordre de prise de vue, comme la photothèque les range.
    let mut entries: Vec<luma_library::Entry> = photos.iter().map(|p| luma_library::entry_of(p)).collect();
    entries.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.path.cmp(&b.path)));
    let fields: Vec<(PathBuf, export::NameFields)> = entries
        .iter()
        .map(|e| {
            let f =
                export::NameFields::of_details(&e.path, &e.date, &e.make, &e.model, &e.lens, e.iso, e.rating, &e.label);
            (e.path.clone(), f)
        })
        .collect();
    let plan = luma_render::rename::plan(&fields, &template)?;
    for m in &plan.photos {
        let name = |p: &Path| {
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        println!("{}\t→ {}", name(&m.from), name(&m.to));
    }
    if dry || plan.files.is_empty() {
        if plan.files.is_empty() {
            eprintln!("rien à renommer");
        }
        return Ok(());
    }
    luma_render::rename::apply(&plan)?;
    eprintln!("{} fichiers renommés", plan.files.len());
    if let Some(file) = file.or_else(luma_library::default_path).filter(|f| f.is_file()) {
        let mut catalog = luma_library::Catalog::load(&file);
        for m in &plan.photos {
            catalog.rename(&m.from, &m.to);
        }
        catalog.save(&file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Une photo et ses réglages : ceux de son XMP, sinon ceux de départ.
fn open_with_settings(path: &Path) -> Result<(Photo, Settings), String> {
    let photo = Photo::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let settings = match sidecar(path) {
        Some(x) => xmp::read(&std::fs::read_to_string(&x).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?,
        None => photo.default_settings(),
    };
    Ok((photo, settings))
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Un nombre d'une option, dans ces bornes.
fn bounded<T: std::str::FromStr + PartialOrd>(option: &str, v: Option<String>, lo: T, hi: T) -> Result<T, String> {
    v.and_then(|v| v.replace(',', ".").parse::<T>().ok())
        .filter(|v| *v >= lo && *v <= hi)
        .ok_or_else(|| format!("{option} : valeur hors des bornes"))
}

/// `luma geotag` : les photos placées d'après une trace GPX.
fn geotag(args: Vec<String>) -> Result<(), String> {
    use luma_library::geotag::{self, Outcome};
    let (mut gpx, mut offset, mut max_gap, mut replace, mut dry) = (None, Some(0), 3600, false, false);
    let mut inputs = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--gpx" => gpx = Some(PathBuf::from(it.next().ok_or("--gpx attend une trace")?)),
            "--offset" => {
                let v = it.next().ok_or("--offset attend un écart (+02:00)")?;
                offset = if v == "auto" {
                    None
                } else {
                    Some(geotag::offset(&v).ok_or_else(|| format!("--offset : « {v} » n'est pas un écart (+02:00)"))?)
                };
            }
            "--max-gap" => max_gap = bounded("--max-gap", it.next(), 1, 86_400 * 7)?,
            "--replace" => replace = true,
            "--dry-run" => dry = true,
            s if s.starts_with('-') => return Err(format!("option inconnue « {s} »")),
            _ => inputs.push(PathBuf::from(a)),
        }
    }
    let gpx = gpx.ok_or("il faut une trace (--gpx trace.gpx)")?;
    let text = std::fs::read_to_string(&gpx).map_err(|e| format!("{} : {e}", gpx.display()))?;
    let track = geotag::read_gpx(&text).map_err(|e| format!("{} : {e}", gpx.display()))?;
    // Les photos : celles données, et celles des dossiers donnés.
    let mut photos = Vec::new();
    for p in inputs {
        if p.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(&p)
                .map_err(|e| format!("{} : {e}", p.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|f| f.is_file() && luma_raw::is_photo(f))
                .collect();
            found.sort();
            photos.extend(found);
        } else {
            photos.push(p);
        }
    }
    if photos.is_empty() {
        return Err("aucune photo à placer".into());
    }
    // --offset auto : l'écart qui fait tomber le plus de photos dans la trace.
    let offset = match offset {
        Some(o) => o,
        None => {
            let times: Vec<i64> = photos
                .iter()
                .filter_map(|p| geotag::parse_time(&luma_library::entry_of(p).date))
                .collect();
            let o = geotag::best_offset(&track, &times, 300, max_gap.min(900)).unwrap_or(0);
            let a = o.abs();
            println!(
                "horloge du boîtier : UTC{}{:02}:{:02} (devinée)",
                if o < 0 { '-' } else { '+' },
                a / 3600,
                a % 3600 / 60
            );
            o
        }
    };
    let (mut placed, mut kept, mut outside, mut undated) = (0, 0, 0, 0);
    for p in &photos {
        match geotag::place(p, &track, offset, max_gap, replace, !dry)? {
            Outcome::Placed(pos) => {
                placed += 1;
                println!("{} : {:.6}, {:.6}", p.display(), pos.latitude, pos.longitude);
            }
            Outcome::AlreadyPlaced => kept += 1,
            Outcome::OutsideTrack => {
                outside += 1;
                println!("{} : hors de la trace", p.display());
            }
            Outcome::NoDate => undated += 1,
        }
    }
    println!(
        "{placed} photos placées{}, {kept} déjà placées, {outside} hors de la trace, {undated} sans date ({} points de trace)",
        if dry { " (essai : rien d'écrit)" } else { "" },
        track.len()
    );
    Ok(())
}

/// `luma video` : un film ou des réglages sur chaque image d'une vidéo.
fn video(args: Vec<String>) -> Result<(), String> {
    use luma_render::video;
    // --film se lit ici, le reste comme pour develop.
    let (mut film, mut rest) = (None, Vec::new());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--film" {
            film = Some(it.next().ok_or("--film attend un nom")?);
        } else {
            rest.push(a);
        }
    }
    let a = parse(rest.into_iter())?;
    let [input] = a.positional.as_slice() else {
        return Err("il faut une vidéo (une seule)".into());
    };
    let input = Path::new(input);
    let out = a.output.clone().ok_or("il faut un fichier de sortie (-o sortie.avi)")?;
    if out == input {
        return Err("la sortie doit être un autre fichier que la vidéo".into());
    }
    let bytes = std::fs::read(input).map_err(|e| format!("{} : {e}", input.display()))?;
    let mut clip = video::read(&bytes).map_err(|e| format!("{} : {e}", input.display()))?;
    drop(bytes);
    // Les réglages : ceux d'une image déjà développée, sauf réglages complets donnés.
    let mut settings = if a.xmp.is_some() || a.settings.is_some() {
        load_settings(&a, None)?
    } else {
        let mut s = Settings::for_rendered();
        for set in &a.sets {
            s.set(set)?;
        }
        // --lut : lu comme pour develop, posé sur ces réglages.
        if a.lut.is_some() {
            s.lut = load_settings(&a, None)?.lut;
        }
        s
    };
    if let Some(spec) = &film {
        let (name, amount) = match spec.rsplit_once(':') {
            Some((n, v)) if v.replace(',', ".").parse::<f32>().is_ok() => {
                (n, v.replace(',', ".").parse::<f32>().unwrap_or(100.0))
            }
            _ => (spec.as_str(), 100.0),
        };
        let preset = luma_project::presets::find(name)
            .ok_or_else(|| format!("film inconnu « {name} » (luma presets les liste tous)"))?;
        settings = preset.applied(&settings, (amount / 100.0).clamp(0.0, 1.0));
    }
    let quality = a.quality.unwrap_or(90).clamp(1, 100);
    let start = Instant::now();
    let total = clip.frame_count();
    video::develop(
        &mut clip,
        quality,
        &Cancel::new(),
        &mut |raw| {
            Photo::new(raw)
                .render(&settings, Size::Full, &Cancel::new())
                .map(|r| r.image)
                .map_err(|e| e.to_string())
        },
        &mut |k, n| {
            if k % 25 == 0 || k == n {
                eprint!("\rimage {k} sur {n}");
            }
        },
    )?;
    eprintln!();
    let written = video::write(&clip)?;
    std::fs::write(&out, &written).map_err(|e| format!("{} : {e}", out.display()))?;
    println!(
        "{} : {total} images ({} × {}, {:.2} images/s, {:.1} s), {:.1} Mo, en {:.1} s",
        out.display(),
        clip.width,
        clip.height,
        clip.fps(),
        clip.seconds(),
        written.len() as f64 / 1e6,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Le livre photo, en PDF.
fn book(args: Vec<String>) -> Result<(), String> {
    use luma_render::book::{self, Book};
    use luma_render::print;
    let mut b = Book::default();
    let (mut out, mut profile, mut photos) = (None, None, Vec::new());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(it.next().ok_or("-o : fichier attendu")?)),
            "--title" => b.title = it.next().unwrap_or_default(),
            "--subtitle" => b.subtitle = it.next().unwrap_or_default(),
            // « \n » dans le texte : une nouvelle ligne.
            "--opening" => b.opening = it.next().unwrap_or_default().replace("\\n", "\n"),
            "--closing" => b.closing = it.next().unwrap_or_default().replace("\\n", "\n"),
            "--pages" => {
                b.rhythm = book::parse_rhythm(&it.next().unwrap_or_default()).map_err(|e| format!("--pages : {e}"))?
            }
            "--format" | "--paper" => {
                let v = it.next().unwrap_or_default();
                let n = v.to_lowercase().replace(['×', ' '], "x");
                b.paper = book::FORMATS
                    .iter()
                    .find(|f| f.name.to_lowercase() == n)
                    .copied()
                    .or_else(|| print::paper(&v))
                    .ok_or_else(|| format!("--format : format inconnu « {v} »"))?;
            }
            "--landscape" => b.landscape = true,
            "--margin" => b.margin = bounded("--margin", it.next(), 0.0, 100.0)?,
            "--gap" => b.gap = bounded("--gap", it.next(), 0.0, 50.0)?,
            "--dpi" => b.dpi = bounded("--dpi", it.next(), 72, 1200)?,
            "--captions" => b.captions = true,
            "--profile" => profile = Some(PathBuf::from(it.next().ok_or("--profile : fichier ICC attendu")?)),
            _ => photos.push(PathBuf::from(a)),
        }
    }
    let out = out.ok_or("il faut un fichier de sortie (-o livre.pdf)")?;
    if photos.is_empty() {
        return Err("aucune photo pour le livre".into());
    }
    if b.cells(book::Kind::Four, true).is_empty() {
        return Err("les marges ne laissent plus de place sur la page".into());
    }
    let printer = match &profile {
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("{} : {e}", p.display()))?;
            let profile =
                luma_color::profile::Profile::parse(&bytes).map_err(|e| format!("{} : {}", p.display(), e.0))?;
            Some(
                luma_color::proof::Conversion::new(&profile, luma_color::profile::Intent::Relative)
                    .map_err(|e| e.0.to_owned())?,
            )
        }
        None => None,
    };
    let start = Instant::now();
    // L'orientation de chaque photo décide des pages.
    let mut landscape = Vec::with_capacity(photos.len());
    for p in &photos {
        let (photo, settings) = open_with_settings(p)?;
        landscape.push(book::is_landscape(&photo, &settings));
    }
    let layout = b.layout();
    let mut pages = Vec::new();
    for sheet in b.sheets(&landscape) {
        let orientation = sheet.photos.first().is_none_or(|&k| landscape[k]);
        let cells = b.cells(sheet.kind, orientation);
        let mut pictures = Vec::with_capacity(sheet.photos.len());
        for (&k, cell) in sheet.photos.iter().zip(&cells) {
            let (photo, settings) = open_with_settings(&photos[k])?;
            pictures.push(print::develop(
                &photo,
                &settings,
                &layout,
                cell,
                file_label(&photos[k]),
            )?);
        }
        pages.push(print::encode(&b.page(sheet.kind, &cells, &pictures, printer.as_ref()))?);
    }
    std::fs::write(&out, print::pdf_of(&layout, &pages)).map_err(|e| format!("{} : {e}", out.display()))?;
    let (w, h) = layout.page_mm();
    println!(
        "{} : livre de {} page(s) de {w:.0} × {h:.0} mm, {} photo(s), en {:.1} s",
        out.display(),
        pages.len(),
        photos.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

/// La galerie web statique.
fn gallery(args: Vec<String>) -> Result<(), String> {
    use luma_render::gallery::{self, Gallery};
    let mut g = Gallery::default();
    let (mut out, mut photos) = (None, Vec::new());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(it.next().ok_or("-o : dossier attendu")?)),
            "--title" => g.title = it.next().unwrap_or_default(),
            "--description" => g.description = it.next().unwrap_or_default(),
            "--size" => g.size = bounded("--size", it.next(), 320, 8192)?,
            "--thumb" => g.thumb = bounded("--thumb", it.next(), 80, 2048)?,
            "--quality" => g.quality = bounded("--quality", it.next(), 1, 100)?,
            "--no-captions" => g.captions = false,
            _ => photos.push(PathBuf::from(a)),
        }
    }
    let out = out.ok_or("il faut un dossier de sortie (-o galerie)")?;
    if photos.is_empty() {
        return Err("aucune photo pour la galerie".into());
    }
    let start = Instant::now();
    let mut entries = Vec::with_capacity(photos.len());
    for p in &photos {
        let (photo, settings) = open_with_settings(p)?;
        let r = photo
            .render(&settings, Size::Fit(g.size), &Cancel::new())
            .map_err(|e| format!("{} : {e}", p.display()))?;
        // La légende : le titre de la photo (IPTC), sinon son nom.
        let title = sidecar(p)
            .and_then(|x| std::fs::read_to_string(x).ok())
            .map(|t| xmp::read_meta(&t).title)
            .filter(|t| !t.trim().is_empty());
        entries.push(gallery::entry(&r.image, &g, title.unwrap_or_else(|| file_label(p)))?);
    }
    gallery::write(&out, &g, &entries)?;
    let bytes: usize = entries.iter().map(|e| e.photo.len() + e.thumb.len()).sum();
    println!(
        "{} : galerie de {} photo(s), {:.1} Mo, en {:.1} s (ouvrir index.html)",
        out.display(),
        entries.len(),
        bytes as f64 / 1e6,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Le diaporama autonome.
fn slideshow(args: Vec<String>) -> Result<(), String> {
    use luma_render::slideshow::{self, Slideshow};
    let mut show = Slideshow::default();
    let (mut out, mut photos) = (None, Vec::new());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(it.next().ok_or("-o : dossier attendu")?)),
            "--title" => show.title = it.next().unwrap_or_default(),
            "--seconds" => show.seconds = bounded("--seconds", it.next(), 0.5, 600.0)?,
            "--fade" => show.fade = bounded("--fade", it.next(), 0.0, 5.0)?,
            "--size" => show.size = bounded("--size", it.next(), 320, 8192)?,
            "--quality" => show.quality = bounded("--quality", it.next(), 1, 100)?,
            "--captions" => show.captions = true,
            "--transition" => {
                let t = it.next().unwrap_or_default();
                show.transition = slideshow::Transition::from_key(&t)
                    .ok_or(format!("--transition : fade, slide ou drift attendu, pas « {t} »"))?;
            }
            "--music" => {
                let m = PathBuf::from(it.next().ok_or("--music : fichier attendu")?);
                show.music = Some(m.clone());
                if slideshow::music_file(&show).is_none() {
                    return Err(format!(
                        "{} : musique en {} attendue",
                        m.display(),
                        slideshow::MUSIC_EXTENSIONS.join(", ")
                    ));
                }
            }
            _ => photos.push(PathBuf::from(a)),
        }
    }
    let out = out.ok_or("il faut un dossier de sortie (-o diaporama)")?;
    if photos.is_empty() {
        return Err("aucune photo pour le diaporama".into());
    }
    let start = Instant::now();
    let mut frames = Vec::with_capacity(photos.len());
    for p in &photos {
        let (photo, settings) = open_with_settings(p)?;
        let r = photo
            .render(&settings, Size::Fit(show.size), &Cancel::new())
            .map_err(|e| format!("{} : {e}", p.display()))?;
        let rgb = luma_image::encode8(&r.image, OutputSpace::Srgb);
        let jpeg = luma_render::jpeg::encode(&rgb, r.image.width, r.image.height, show.quality, &[], &[])?;
        frames.push((jpeg, file_label(p)));
    }
    slideshow::write(&out, &show, &frames)?;
    println!(
        "{} : diaporama de {} photo(s), en {:.1} s (ouvrir index.html)",
        out.display(),
        frames.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

/// La mise en page pour l'imprimante, en PDF.
fn print(args: Vec<String>) -> Result<(), String> {
    use luma_color::profile::{Intent, Profile};
    use luma_render::print::{self, Layout};
    let mut layout = Layout::default();
    let (mut out, mut profile, mut intent, mut photos) = (None, None, Intent::Relative, Vec::new());
    let mut it = args.into_iter();
    let number = |o: &str, v: Option<String>| -> Result<f32, String> {
        v.and_then(|v| v.replace(',', ".").parse::<f32>().ok())
            .filter(|v| (0.0..=500.0).contains(v))
            .ok_or_else(|| format!("{o} : un nombre de millimètres"))
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => out = Some(PathBuf::from(it.next().ok_or("-o : fichier attendu")?)),
            "--paper" => {
                let v = it.next().unwrap_or_default();
                layout.paper = print::paper(&v).ok_or_else(|| format!("--paper : format inconnu « {v} »"))?;
            }
            "--landscape" => layout.landscape = true,
            "--margin" => layout.margin = number("--margin", it.next())?,
            "--gap" => layout.gap = number("--gap", it.next())?,
            "--grid" => {
                let v = it.next().unwrap_or_default();
                let (c, r) = v
                    .split_once(['x', '×'])
                    .and_then(|(c, r)| Some((c.parse::<usize>().ok()?, r.parse::<usize>().ok()?)))
                    .filter(|&(c, r)| (1..=20).contains(&c) && (1..=20).contains(&r))
                    .ok_or("--grid : colonnes x lignes, de 1 à 20 (4x5)")?;
                (layout.columns, layout.rows) = (c, r);
            }
            "--dpi" => {
                layout.dpi = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|d| (72..=1200).contains(d))
                    .ok_or("--dpi : de 72 à 1200")?;
            }
            "--captions" => layout.captions = true,
            "--fill" => layout.fill = true,
            "--crop-marks" => layout.crop_marks = true,
            "--text" => layout.text = it.next().ok_or("--text attend un texte")?,
            "--profile" => profile = Some(PathBuf::from(it.next().ok_or("--profile : fichier ICC attendu")?)),
            "--intent" => {
                intent = match it.next().as_deref() {
                    Some("perceptual" | "perceptuelle") => Intent::Perceptual,
                    Some("relative") => Intent::Relative,
                    Some("saturation") => Intent::Saturation,
                    Some("absolute" | "absolue") => Intent::Absolute,
                    _ => return Err("--intent : perceptual, relative, saturation ou absolute".into()),
                }
            }
            "--output-sharpen" => {
                let v = it.next().unwrap_or_default();
                layout.sharpen = Some(export::OutputSharpen::parse(&v).ok_or_else(|| sharpen_error(&v))?);
            }
            _ => photos.push(PathBuf::from(a)),
        }
    }
    let out = out.ok_or("il faut un fichier de sortie (-o tirage.pdf)")?;
    if photos.is_empty() {
        return Err("aucune photo à imprimer".into());
    }
    let cells = layout.cells();
    if cells.is_empty() {
        return Err("les marges ne laissent plus de place sur le papier".into());
    }
    let printer = match &profile {
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| format!("{} : {e}", p.display()))?;
            let profile = Profile::parse(&bytes).map_err(|e| format!("{} : {}", p.display(), e.0))?;
            Some(luma_color::proof::Conversion::new(&profile, intent).map_err(|e| e.0.to_owned())?)
        }
        None => None,
    };
    let start = Instant::now();
    // Page par page : développées, posées, compressées.
    let mut pages = Vec::new();
    for chunk in photos.chunks(cells.len()) {
        let mut pictures = Vec::with_capacity(chunk.len());
        for (path, cell) in chunk.iter().zip(&cells) {
            let photo = Photo::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
            let settings = match sidecar(path) {
                Some(x) => {
                    xmp::read(&std::fs::read_to_string(&x).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?
                }
                None => photo.default_settings(),
            };
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            pictures.push(print::develop(&photo, &settings, &layout, cell, name)?);
        }
        for page in print::compose(&layout, &pictures, printer.as_ref()) {
            pages.push(print::encode(&page)?);
        }
    }
    let pdf = print::pdf_of(&layout, &pages);
    std::fs::write(&out, pdf).map_err(|e| format!("{} : {e}", out.display()))?;
    let (w, h) = layout.page_mm();
    println!(
        "{} : {} page(s) de {w:.0} × {h:.0} mm, {} photo(s), en {:.1} s",
        out.display(),
        pages.len(),
        photos.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

/// L'import d'une carte mémoire ou d'un dossier.
fn import(args: Vec<String>) -> Result<(), String> {
    use luma_library::import::{self, Options};
    let mut o = Options {
        dest: PathBuf::new(),
        folders: "{année}/{année}-{mois}-{jour}".into(),
        backup: None,
        days: Vec::new(),
        skip: Vec::new(),
    };
    let (mut source, mut dest, mut template, mut dry, mut file) = (None, None, None, false, None);
    let mut eject = false;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--to" => dest = Some(PathBuf::from(it.next().ok_or("--to : dossier attendu")?)),
            "--folders" => o.folders = it.next().unwrap_or_default(),
            "--name" => template = Some(it.next().ok_or("--name : modèle attendu")?),
            "--backup" => o.backup = Some(PathBuf::from(it.next().ok_or("--backup : dossier attendu")?)),
            "--dry-run" | "-n" => dry = true,
            "--days" => {
                let v = it.next().ok_or("--days : des jours attendus (2026-10-26,2026-10-27)")?;
                o.days = v
                    .split(',')
                    .map(|d| d.trim().to_owned())
                    .filter(|d| !d.is_empty())
                    .collect();
                if let Some(bad) = o
                    .days
                    .iter()
                    .find(|d| luma_library::geotag::parse_time(&format!("{d}T00:00:00")).is_none())
                {
                    return Err(format!("--days : « {bad} » n'est pas un jour (2026-10-26)"));
                }
            }
            "--catalog" => file = Some(PathBuf::from(it.next().ok_or("--catalog : fichier attendu")?)),
            "--eject" => eject = true,
            "--skip" => o.skip.push(PathBuf::from(it.next().ok_or("--skip : photo attendue")?)),
            _ => source = Some(PathBuf::from(a)),
        }
    }
    o.dest = dest.ok_or("il faut le dossier de la photothèque (--to ~/Photos)")?;
    let source = match source {
        Some(s) => s,
        None => {
            let cards = import::cards();
            match cards.as_slice() {
                [one] => one.clone(),
                [] => return Err("aucune carte mémoire montée (ou donnez le dossier à importer)".into()),
                many => {
                    let list: Vec<String> = many.iter().map(|c| c.display().to_string()).collect();
                    return Err(format!("plusieurs cartes : {} ; dites laquelle", list.join(", ")));
                }
            }
        }
    };
    if let Some(t) = &template {
        export::check_template(t)?;
    }
    let found = import::photos(&source);
    if found.is_empty() {
        return Err(format!("aucune photo dans {}", source.display()));
    }
    let entries: Vec<luma_library::Entry> = found.iter().map(|p| luma_library::entry_of(p)).collect();
    let days: Vec<String> = import::days(&entries)
        .iter()
        .map(|(d, n)| format!("{d} ({n})"))
        .collect();
    eprintln!("jours : {}", days.join(", "));
    let plan = import::plan(&entries, &o, |e, n| {
        let t = template.as_ref()?;
        let f = export::NameFields::of_details(&e.path, &e.date, &e.make, &e.model, &e.lens, e.iso, e.rating, &e.label);
        let fields = export::NameFields { sequence: n, ..f };
        Some(export::file_name(t, &fields))
    });
    for item in &plan.items {
        println!("{}\t→ {}", item.source.display(), item.target.display());
    }
    eprintln!(
        "{} photo(s) à importer ({:.1} Mo), {} déjà importée(s)",
        plan.items.len(),
        plan.bytes() as f64 / 1e6,
        plan.skipped.len()
    );
    if dry || plan.items.is_empty() {
        return Ok(());
    }
    let start = Instant::now();
    let report = import::run(&plan, |n| eprint!("\r{n} / {}", plan.items.len()))?;
    eprintln!(
        "\r{} photo(s) importée(s) et vérifiée(s), {:.1} Mo, en {:.1} s{}",
        report.copied,
        report.bytes as f64 / 1e6,
        start.elapsed().as_secs_f64(),
        if o.backup.is_some() {
            ", aussi sur le second disque"
        } else {
            ""
        }
    );
    // La photothèque : les dossiers où les photos sont arrivées.
    if let Some(file) = file.or_else(luma_library::default_path) {
        let mut catalog = luma_library::Catalog::load(&file);
        let mut dirs: Vec<PathBuf> = plan
            .items
            .iter()
            .filter_map(|i| i.target.parent().map(Path::to_path_buf))
            .collect();
        dirs.sort();
        dirs.dedup();
        for d in &dirs {
            catalog.add_folder(d, false);
        }
        catalog.save(&file).map_err(|e| e.to_string())?;
    }
    if eject {
        match import::eject(&source) {
            Ok(()) => eprintln!("{} : carte éjectée", source.display()),
            Err(e) => return Err(format!("{} : la carte n'a pas pu être éjectée : {e}", source.display())),
        }
    }
    Ok(())
}

/// Décrit un profil ICC.
fn icc(path: &Path) -> Result<(), String> {
    use luma_color::profile::{Class, Intent, Pcs, Profile, Space, xyz_to_lab};
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let p = Profile::parse(&bytes).map_err(|e| e.to_string())?;
    let class = match p.class {
        Class::Input => "entrée",
        Class::Display => "écran",
        Class::Output => "sortie (imprimante)",
        Class::Link => "liaison",
        Class::ColorSpace => "espace de couleurs",
        Class::Abstract => "abstrait",
        Class::Named => "couleurs nommées",
    };
    let space = match p.space {
        Space::Gray => "niveaux de gris".to_owned(),
        Space::Rgb => "RVB".to_owned(),
        Space::Cmy => "CMJ".to_owned(),
        Space::Cmyk => "CMJN".to_owned(),
        Space::Lab => "Lab".to_owned(),
        Space::Xyz => "XYZ".to_owned(),
        Space::Colors(n) => format!("{n} couleurs"),
    };
    let intent = |i: &Intent| match i {
        Intent::Perceptual => "perceptuelle",
        Intent::Relative => "colorimétrique relative",
        Intent::Saturation => "saturation",
        Intent::Absolute => "colorimétrique absolue",
    };
    let lab = |v: [f32; 3]| {
        // + 0.0 : pas de « -0.0 » à l'affichage.
        let l = xyz_to_lab(v).map(|c| (c * 10.0).round() / 10.0 + 0.0);
        format!("L {:.1}  a {:.1}  b {:.1}", l[0], l[1], l[2])
    };
    println!("{}", path.display());
    println!("  description  {}", p.description);
    println!("  version      {}.{}", p.version.0, p.version.1);
    println!(
        "  classe       {class}, {space} ({} canaux) ↔ {}",
        p.channels(),
        if p.pcs == Pcs::Lab { "Lab" } else { "XYZ" }
    );
    let tables: Vec<&str> = p.tables().iter().map(intent).collect();
    println!(
        "  intentions   {} (en‑tête : {})",
        if tables.is_empty() {
            "matrice".to_owned()
        } else {
            tables.join(", ")
        },
        intent(&p.intent)
    );
    println!("  support      {}", lab(p.white));
    let sens: Vec<&str> = [(p.reads(), "périphérique → XYZ"), (p.writes(), "XYZ → périphérique")]
        .into_iter()
        .filter_map(|(ok, s)| ok.then_some(s))
        .collect();
    println!("  sens         {}", sens.join(", "));
    if !p.reads() || matches!(p.space, Space::Lab | Space::Xyz) {
        return Ok(());
    }
    let n = p.channels();
    // Le blanc et le noir du périphérique : sans encre et toutes encres pour une
    // imprimante soustractive, l'inverse pour un écran.
    let subtractive = matches!(p.space, Space::Cmy | Space::Cmyk | Space::Colors(_));
    let (paper, ink) = if subtractive { (0.0, 1.0) } else { (1.0, 0.0) };
    let mut samples = vec![("blanc".to_owned(), vec![paper; n]), ("noir".to_owned(), vec![ink; n])];
    if matches!(p.space, Space::Rgb | Space::Cmy | Space::Cmyk) {
        let names: &[&str] = match p.space {
            Space::Rgb => &["rouge", "vert", "bleu"],
            Space::Cmy => &["cyan", "magenta", "jaune"],
            _ => &["cyan", "magenta", "jaune", "noir seul"],
        };
        for (i, name) in names.iter().enumerate() {
            let mut v = vec![if subtractive { paper } else { ink }; n];
            v[i] = if subtractive { ink } else { paper };
            samples.push(((*name).to_owned(), v));
        }
    }
    for (name, v) in samples {
        if let Some(xyz) = p.to_pcs(&v, Intent::Relative) {
            println!("  {name:<12} {}", lab(xyz));
        }
    }
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
    if command == "watch" {
        return match watch(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "icc" {
        let mut ok = true;
        for path in args {
            if let Err(e) = icc(Path::new(&path)) {
                eprintln!("erreur : {path} : {e}");
                ok = false;
            }
        }
        return if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
    if command == "book" || command == "slideshow" || command == "gallery" {
        let result = match command.as_str() {
            "book" => book(args.collect()),
            "slideshow" => slideshow(args.collect()),
            _ => gallery(args.collect()),
        };
        return match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "geotag" {
        return match geotag(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "video" {
        return match video(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "print" {
        return match print(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "import" {
        return match import(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("erreur : {e}");
                ExitCode::FAILURE
            }
        };
    }
    if command == "rename" {
        return match rename(args.collect()) {
            Ok(()) => ExitCode::SUCCESS,
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
        "export" => export_many(&parsed),
        "xmp" => to_xmp(&parsed),
        "colorchecker" => colorchecker(&parsed),
        "match-colors" => match_colors(&parsed),
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
