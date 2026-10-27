//! Les piles : les rafales et les séries d'exposition, reconnues à leur date de prise de
//! vue (des photos du même boîtier à quelques secondes d'écart) et à leurs réglages
//! (une série d'exposition change l'exposition d'une photo à l'autre, une rafale non).

use std::path::PathBuf;

use crate::Entry;

/// L'écart, en secondes, au‑delà duquel deux photos ne sont plus de la même pile.
pub const GAP: i64 = 2;

/// L'écart d'exposition (IL) au‑delà duquel une pile est une série d'exposition.
const BRACKET_EV: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Les mêmes réglages : une rafale.
    Burst,
    /// L'exposition change : une série d'exposition (à fusionner en HDR).
    Bracket,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    /// Les photos, dans l'ordre de la prise de vue (la première est la couverture).
    pub photos: Vec<PathBuf>,
    pub kind: Kind,
}

/// Les secondes depuis l'an 0 d'une date `AAAA-MM-JJTHH:MM:SS` (ou avec un espace, ou
/// des fractions, ignorées). `None` si elle ne se lit pas.
fn seconds(date: &str) -> Option<i64> {
    let b = date.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |r: std::ops::Range<usize>| date.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, s) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Jours depuis l'an 0 (calendrier grégorien proleptique).
    let (y, m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };
    let days = 365 * y + y / 4 - y / 100 + y / 400 + (153 * (m - 3) + 2) / 5 + d;
    Some(days * 86_400 + h * 3_600 + mi * 60 + s)
}

/// L'exposition d'une photo, en IL ramenés à ISO 100 ; `None` sans ouverture ni vitesse.
fn exposure(e: &Entry) -> Option<f32> {
    let (n, t) = (e.aperture?, e.exposure_time?);
    if n <= 0.0 || t <= 0.0 {
        return None;
    }
    let iso = e.iso.unwrap_or(100).max(1) as f32;
    Some((n * n / t).log2() - (iso / 100.0).log2())
}

/// Les piles de ces photos (deux photos au moins chacune), dans l'ordre de la prise de
/// vue. Les photos sans date ou seules ne sont dans aucune pile.
pub fn stacks<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> Vec<Stack> {
    // Boîtier par boîtier : deux boîtiers qui déclenchent ensemble ne coupent pas leurs
    // rafales.
    let mut by_camera: std::collections::BTreeMap<String, Vec<(i64, &Entry)>> = Default::default();
    for e in entries {
        if let Some(t) = seconds(&e.date) {
            by_camera.entry(e.camera()).or_default().push((t, e));
        }
    }
    let mut out: Vec<(i64, Stack)> = Vec::new();
    for shots in by_camera.values_mut() {
        shots.sort_by(|a, b| (a.0, &a.1.path).cmp(&(b.0, &b.1.path)));
        let mut group: Vec<(i64, &Entry)> = Vec::new();
        for &(t, e) in shots.iter() {
            if group.last().is_some_and(|(last, _)| t - last > GAP) {
                close(&mut group, &mut out);
            }
            group.push((t, e));
        }
        close(&mut group, &mut out);
    }
    out.sort_by(|a, b| (a.0, &a.1.photos).cmp(&(b.0, &b.1.photos)));
    out.into_iter().map(|(_, s)| s).collect()
}

/// Une pile, si ce groupe en fait une (deux photos au moins).
fn close(group: &mut Vec<(i64, &Entry)>, out: &mut Vec<(i64, Stack)>) {
    if group.len() >= 2 {
        let evs: Vec<f32> = group.iter().filter_map(|(_, e)| exposure(e)).collect();
        let high = evs.iter().copied().fold(f32::MIN, f32::max);
        let low = evs.iter().copied().fold(f32::MAX, f32::min);
        let kind = if evs.len() >= 2 && high - low > BRACKET_EV {
            Kind::Bracket
        } else {
            Kind::Burst
        };
        out.push((
            group[0].0,
            Stack {
                photos: group.iter().map(|(_, e)| e.path.clone()).collect(),
                kind,
            },
        ));
    }
    group.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(name: &str, date: &str, t: f32) -> Entry {
        Entry {
            path: PathBuf::from(name),
            date: date.into(),
            make: "Sony".into(),
            model: "ILCE-7M3".into(),
            iso: Some(100),
            aperture: Some(8.0),
            exposure_time: Some(t),
            ..Entry::default()
        }
    }

    #[test]
    fn bursts_and_brackets() {
        let mut other = shot("autre.ARW", "2024-06-15T10:00:01", 0.01);
        other.model = "ILCE-7RM5".into();
        let photos = vec![
            // Une rafale : trois photos dans la même seconde, puis une à la suivante.
            shot("r1.ARW", "2024-06-15T10:00:00", 0.004),
            shot("r2.ARW", "2024-06-15T10:00:00", 0.004),
            shot("r3.ARW", "2024-06-15T10:00:00", 0.004),
            shot("r4.ARW", "2024-06-15T10:00:01", 0.004),
            // Un autre boîtier au même moment : à part.
            other,
            // Une série d'exposition à −2, 0, +2 IL, une minute plus tard.
            shot("b1.ARW", "2024-06-15T10:01:00", 1.0 / 500.0),
            shot("b2.ARW", "2024-06-15T10:01:01", 1.0 / 125.0),
            shot("b3.ARW", "2024-06-15T10:01:02", 1.0 / 30.0),
            // Seule, à dix secondes : dans aucune pile ; sans date non plus.
            shot("seule.ARW", "2024-06-15T10:01:12", 0.01),
            shot("sans-date.ARW", "", 0.01),
        ];
        let s = stacks(&photos);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!(s[0].kind, Kind::Burst);
        assert_eq!(s[0].photos, ["r1.ARW", "r2.ARW", "r3.ARW", "r4.ARW"].map(PathBuf::from));
        assert_eq!(s[1].kind, Kind::Bracket);
        assert_eq!(s[1].photos, ["b1.ARW", "b2.ARW", "b3.ARW"].map(PathBuf::from));
    }

    #[test]
    fn dates_across_midnight_and_months() {
        assert_eq!(
            seconds("2024-03-01T00:00:00").unwrap() - seconds("2024-02-29T23:59:59").unwrap(),
            1
        );
        assert_eq!(
            seconds("2025-01-01 00:00:01").unwrap() - seconds("2024-12-31T23:59:59").unwrap(),
            2
        );
        assert!(seconds("hier").is_none());
        assert!(seconds("2024-13-01T00:00:00").is_none());
        // Juste avant et juste après minuit : la même pile.
        let a = shot("a.ARW", "2024-12-31T23:59:59", 0.01);
        let b = shot("b.ARW", "2025-01-01T00:00:00", 0.01);
        assert_eq!(stacks([&a, &b]).len(), 1);
    }
}
