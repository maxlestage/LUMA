//! Placer les photos d'après une trace GPX (celle d'un téléphone, d'une montre ou d'un
//! GPS de randonnée) : chaque photo prend la position de la trace à l'instant de sa
//! prise de vue, entre les deux points qui l'encadrent. La position s'écrit dans le XMP
//! de la photo (`exif:GPSLatitude`, `exif:GPSLongitude`) ; le fichier n'est pas touché.
//!
//! Les boîtiers datent leurs photos à l'heure de leur horloge, sans fuseau : l'écart à
//! l'heure universelle (`+02:00` pour l'heure d'été de Paris) se donne à part. Une photo
//! prise hors de la trace, ou pendant un trou trop long (le GPS éteint), n'est pas placée.

use std::path::{Path, PathBuf};

use luma_project::xmp::{self, Position};
use quick_xml::XmlVersion;

/// Un point de la trace : l'instant (secondes depuis 1970, heure universelle) et la
/// position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub time: i64,
    pub latitude: f64,
    pub longitude: f64,
}

/// Les jours depuis le 1ᵉʳ janvier 1970 d'une date du calendrier grégorien.
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Un instant `AAAA-MM-JJTHH:MM:SS` (fraction de seconde et fuseau facultatifs :
/// `Z`, `+02:00`, `-0500`), en secondes depuis 1970, heure universelle. Sans fuseau,
/// l'heure est lue telle quelle.
pub fn parse_time(text: &str) -> Option<i64> {
    let t = text.trim();
    let b = t.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<i64> { t.get(r)?.parse().ok() };
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b' ') {
        return None;
    }
    let (y, mo, d, h, mi, s) = (
        num(0..4)?,
        num(5..7)?,
        num(8..10)?,
        num(11..13)?,
        num(14..16)?,
        num(17..19)?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    let mut rest = &t[19..];
    if let Some(r) = rest.strip_prefix('.') {
        rest = r.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match rest {
        "" | "Z" | "z" => 0,
        r => offset(r)?,
    };
    Some(days(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - offset)
}

/// Un écart à l'heure universelle, en secondes : `+02:00`, `-05:30`, `+0200`, `+2`.
pub fn offset(text: &str) -> Option<i64> {
    let t = text.trim();
    let (sign, body) = match t.as_bytes().first()? {
        b'+' => (1, &t[1..]),
        b'-' => (-1, &t[1..]),
        _ => (1, t),
    };
    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<i64>().ok()?, m.parse::<i64>().ok()?),
        None if body.len() == 4 => (body[..2].parse().ok()?, body[2..].parse().ok()?),
        None => (body.parse().ok()?, 0),
    };
    (h <= 14 && m < 60).then_some(sign * (h * 3600 + m * 60))
}

/// Les points d'une trace GPX (`trkpt`, et `rtept` ou `wpt` datés), dans l'ordre du
/// temps.
pub fn read_gpx(text: &str) -> Result<Vec<Point>, String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(text);
    let mut points = Vec::new();
    let mut current: Option<(f64, f64)> = None;
    let mut in_time = false;
    let attr = |e: &quick_xml::events::BytesStart, name: &str| -> Option<f64> {
        e.attributes()
            .flatten()
            .find(|a| {
                let key: &str = a.key.as_ref();
                key == name
            })
            .and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok()?.trim().parse().ok())
    };
    loop {
        match reader.read_event().map_err(|e| format!("GPX illisible : {e}"))? {
            Event::Start(e) => match e.local_name().as_ref() {
                "trkpt" | "rtept" | "wpt" => {
                    current = attr(&e, "lat").zip(attr(&e, "lon"));
                }
                "time" => in_time = current.is_some(),
                _ => {}
            },
            Event::Text(t) if in_time => {
                let text = t.xml_content(XmlVersion::Implicit1_0);
                if let (Some((latitude, longitude)), Some(time)) = (current, parse_time(&text)) {
                    points.push(Point {
                        time,
                        latitude,
                        longitude,
                    });
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                "time" => in_time = false,
                "trkpt" | "rtept" | "wpt" => current = None,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    points.retain(|p| p.latitude.abs() <= 90.0 && p.longitude.abs() <= 180.0);
    points.sort_by_key(|p| p.time);
    if points.is_empty() {
        return Err("aucun point daté dans cette trace GPX".into());
    }
    Ok(points)
}

/// La position de la trace à l'instant `time` : entre les deux points qui l'encadrent
/// (s'ils ne sont pas séparés de plus de `max_gap` secondes), ou au premier ou au
/// dernier point à moins de `max_gap` secondes.
pub fn position_at(track: &[Point], time: i64, max_gap: i64) -> Option<Position> {
    let at = |p: &Point| Position {
        latitude: p.latitude,
        longitude: p.longitude,
    };
    let i = track.partition_point(|p| p.time <= time);
    match (i.checked_sub(1).map(|k| &track[k]), track.get(i)) {
        (Some(a), _) if a.time == time => Some(at(a)),
        (Some(a), Some(b)) => {
            if b.time - a.time > max_gap {
                // Un trou dans la trace : le point le plus proche, s'il l'est assez.
                let near = if time - a.time <= b.time - time { a } else { b };
                return ((near.time - time).abs() <= max_gap).then(|| at(near));
            }
            let t = (time - a.time) as f64 / (b.time - a.time) as f64;
            Some(Position {
                latitude: a.latitude + (b.latitude - a.latitude) * t,
                longitude: a.longitude + (b.longitude - a.longitude) * t,
            })
        }
        (Some(a), None) => (time - a.time <= max_gap).then(|| at(a)),
        (None, Some(b)) => (b.time - time <= max_gap).then(|| at(b)),
        (None, None) => None,
    }
}

/// Ce que fait le placement d'une photo.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Placed(Position),
    /// Elle a déjà une position (sauf si on la remplace).
    AlreadyPlaced,
    /// Sa date est hors de la trace, ou dans un trou.
    OutsideTrack,
    /// Pas de date de prise de vue lisible.
    NoDate,
}

/// L'instant de la prise de vue d'une photo du catalogue (`AAAA-MM-JJTHH:MM:SS`, à
/// l'heure du boîtier), ramené à l'heure universelle par `camera_offset` secondes.
pub fn photo_time(date: &str, camera_offset: i64) -> Option<i64> {
    parse_time(date).map(|t| t - camera_offset)
}

/// Place une photo d'après la trace : sa date de prise de vue (l'heure du boîtier, à
/// `camera_offset` secondes de l'heure universelle), sa position sur la trace, écrite
/// dans son XMP si `write`. Une photo qui a déjà une position la garde, sauf `replace`.
pub fn place(
    photo: &Path,
    track: &[Point],
    camera_offset: i64,
    max_gap: i64,
    replace: bool,
    write: bool,
) -> Result<Outcome, String> {
    let e = crate::entry_of(photo);
    if e.gps.is_some() && !replace {
        return Ok(Outcome::AlreadyPlaced);
    }
    let Some(time) = photo_time(&e.date, camera_offset) else {
        return Ok(Outcome::NoDate);
    };
    let Some(p) = position_at(track, time, max_gap) else {
        return Ok(Outcome::OutsideTrack);
    };
    if write {
        write_position(photo, p)?;
    }
    Ok(Outcome::Placed(p))
}

/// Écrit la position dans le XMP de la photo, en gardant ses réglages et ses
/// informations ; sans XMP, un XMP de métadonnées seules est créé.
pub fn write_position(photo: &Path, position: Position) -> Result<PathBuf, String> {
    let target = xmp::sidecar_path(photo);
    let text = std::fs::read_to_string(&target).unwrap_or_default();
    let mut meta = xmp::read_meta(&text);
    meta.position = Some(position);
    let out = if text.contains("crs:") {
        let settings = xmp::read(&text).map_err(|e| format!("{} : {e}", target.display()))?;
        xmp::write_with_meta(&settings, &meta)
    } else {
        xmp::write_meta(&meta)
    };
    std::fs::write(&target, out).map_err(|e| format!("{} : {e}", target.display()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="essai" xmlns="http://www.topografix.com/GPX/1/1">
 <trk><name>Paris</name><trkseg>
  <trkpt lat="48.8584" lon="2.2945"><ele>35</ele><time>2026-06-01T08:00:00Z</time></trkpt>
  <trkpt lat="48.8606" lon="2.3376"><time>2026-06-01T08:10:00.500Z</time></trkpt>
  <trkpt lat="48.8530" lon="2.3499"><time>2026-06-01T10:30:00+02:00</time></trkpt>
  <trkpt lat="48.8867" lon="2.3431"><time>2026-06-01T12:00:00Z</time></trkpt>
 </trkseg></trk>
</gpx>"#;

    #[test]
    fn times_and_offsets() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2000-03-01T00:00:00"), Some(951_868_800));
        assert_eq!(
            parse_time("2026-06-01T10:00:00+02:00"),
            parse_time("2026-06-01T08:00:00Z")
        );
        assert_eq!(
            parse_time("2026-06-01T03:00:00-0500"),
            parse_time("2026-06-01T08:00:00Z")
        );
        assert_eq!(
            parse_time("2026-06-01 08:00:00.250Z"),
            parse_time("2026-06-01T08:00:00Z")
        );
        assert_eq!(parse_time("hier"), None);
        assert_eq!(parse_time("2026-13-01T08:00:00Z"), None);
        assert_eq!(
            (offset("+02:00"), offset("-05:30"), offset("+0100"), offset("3")),
            (Some(7200), Some(-19_800), Some(3600), Some(10_800))
        );
        assert_eq!(offset("+25:00"), None);
    }

    #[test]
    fn the_track_is_read_in_time_order() {
        let t = read_gpx(TRACE).unwrap();
        assert_eq!(t.len(), 4);
        assert!(t.windows(2).all(|w| w[0].time < w[1].time));
        // 10:30 à Paris, en été : 08:30 en heure universelle.
        assert_eq!(t[2].time, parse_time("2026-06-01T08:30:00Z").unwrap());
        assert!(read_gpx("<gpx></gpx>").is_err());
        assert!(
            read_gpx("<gpx><trk><trkpt lat=\"1\" lon=\"2\"></trkpt></trk></gpx>").is_err(),
            "sans date"
        );
    }

    #[test]
    fn positions_along_the_track() {
        let t = read_gpx(TRACE).unwrap();
        let at = |s: &str| position_at(&t, parse_time(s).unwrap(), 3600);
        // Sur un point, entre deux points (au milieu), avant et après la trace.
        let p = at("2026-06-01T08:00:00Z").unwrap();
        assert_eq!((p.latitude, p.longitude), (48.8584, 2.2945));
        let mid = at("2026-06-01T08:20:00Z").unwrap();
        assert!((mid.latitude - (48.8606 + 48.8530) / 2.0).abs() < 1e-3, "{mid:?}");
        assert!(
            at("2026-06-01T07:30:00Z").is_some(),
            "une demi-heure avant : le premier point"
        );
        assert!(at("2026-06-01T06:00:00Z").is_none(), "deux heures avant : rien");
        assert!(at("2026-06-01T14:00:00Z").is_none());
        // Un trou de trois heures et demie (08:30 → 12:00) : près de ses bords seulement.
        let gap = position_at(&t, parse_time("2026-06-01T08:40:00Z").unwrap(), 1800).unwrap();
        assert_eq!((gap.latitude, gap.longitude), (48.8530, 2.3499));
        assert!(position_at(&t, parse_time("2026-06-01T10:15:00Z").unwrap(), 1800).is_none());
        // L'heure du boîtier, réglé à l'heure de Paris.
        let local = photo_time("2026-06-01T10:20:00", offset("+02:00").unwrap()).unwrap();
        assert_eq!(position_at(&t, local, 3600), Some(mid));
    }

    #[test]
    fn the_position_goes_into_the_xmp() {
        let dir = std::env::temp_dir().join(format!("luma-geotag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (raw, developed) = (dir.join("IMG_0001.CR3"), dir.join("IMG_0002.CR3"));
        // Une photo déjà développée : ses réglages et sa note restent.
        let settings = luma_project::Settings {
            exposure: 0.7,
            ..luma_project::Settings::default()
        };
        let meta = xmp::Meta {
            rating: 4,
            ..xmp::Meta::default()
        };
        std::fs::write(xmp::sidecar_path(&developed), xmp::write_with_meta(&settings, &meta)).unwrap();
        let p = Position {
            latitude: -33.8568,
            longitude: 151.2153,
        };
        for photo in [&raw, &developed] {
            let side = write_position(photo, p).unwrap();
            let text = std::fs::read_to_string(side).unwrap();
            let m = xmp::read_meta(&text);
            let q = m.position.unwrap();
            assert!((q.latitude - p.latitude).abs() < 1e-8 && (q.longitude - p.longitude).abs() < 1e-8);
            if photo == &developed {
                assert_eq!(m.rating, 4);
                assert_eq!(xmp::read(&text).unwrap().exposure, 0.7);
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
