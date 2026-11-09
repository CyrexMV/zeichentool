//! Schule & Ausbildung: Prüfungs-Countdown und Notenübersicht (Daten + Speicherformat).

/// Eine Prüfung: Tag (seit 1970, lokal) und Name.
#[derive(Clone, Debug, PartialEq)]
pub struct Exam {
    pub day: i64,
    pub name: String,
}

/// Ein Fach mit Noten (Note 1,0–6,0, Gewicht 1 oder 2).
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    pub name: String,
    pub grades: Vec<(f32, u8)>,
}

impl Subject {
    /// Gewichteter Durchschnitt oder `None` ohne Noten.
    pub fn average(&self) -> Option<f32> {
        let w: u32 = self.grades.iter().map(|g| g.1.max(1) as u32).sum();
        if w == 0 {
            return None;
        }
        Some(self.grades.iter().map(|g| g.0 * g.1.max(1) as f32).sum::<f32>() / w as f32)
    }
}

/// Gesamtschnitt = Mittel der Fachdurchschnitte (Fächer ohne Noten zählen nicht).
pub fn overall(subjects: &[Subject]) -> Option<f32> {
    let avgs: Vec<f32> = subjects.iter().filter_map(|s| s.average()).collect();
    if avgs.is_empty() {
        None
    } else {
        Some(avgs.iter().sum::<f32>() / avgs.len() as f32)
    }
}

/// Note lesen: „2“, „2,5“, „2.5“, „2+“ (1,7), „2-“ (2,3). Gültig 1,0 bis 6,0.
pub fn parse_grade(s: &str) -> Option<f32> {
    let s = s.trim().replace(',', ".");
    let (num, adj) = if let Some(n) = s.strip_suffix('+') {
        (n.to_string(), -0.3)
    } else if let Some(n) = s.strip_suffix('-').or_else(|| s.strip_suffix('–')) {
        (n.to_string(), 0.3)
    } else {
        (s.clone(), 0.0)
    };
    let v: f32 = num.trim().parse().ok()?;
    let v = ((v + adj) * 10.0).round() / 10.0;
    (1.0..=6.0).contains(&v).then_some(v)
}

/// Note für die Anzeige: „2,3“.
pub fn fmt_grade(v: f32) -> String {
    let s = format!("{:.1}", v);
    s.replace('.', ",")
}

fn clean(s: &str) -> String {
    s.chars().map(|c| if matches!(c, ';' | '|' | ':' | ',' | '\n' | '\r' | '\t') { ' ' } else { c }).collect::<String>().trim().to_string()
}

pub fn encode_exams(v: &[Exam]) -> String {
    v.iter().map(|e| format!("{}|{}", e.day, clean(&e.name))).collect::<Vec<_>>().join(";")
}

pub fn decode_exams(s: &str) -> Vec<Exam> {
    s.split(';')
        .filter_map(|p| {
            let (d, n) = p.split_once('|')?;
            Some(Exam { day: d.trim().parse().ok()?, name: n.to_string() })
        })
        .take(50)
        .collect()
}

pub fn encode_subjects(v: &[Subject]) -> String {
    v.iter()
        .map(|s| format!("{}:{}", clean(&s.name), s.grades.iter().map(|g| format!("{}*{}", g.0, g.1)).collect::<Vec<_>>().join(",")))
        .collect::<Vec<_>>()
        .join("|")
}

pub fn decode_subjects(s: &str) -> Vec<Subject> {
    s.split('|')
        .filter_map(|p| {
            let (name, gs) = p.split_once(':')?;
            if name.trim().is_empty() {
                return None;
            }
            let grades = gs
                .split(',')
                .filter_map(|g| {
                    let (v, w) = g.split_once('*')?;
                    let v: f32 = v.parse().ok()?;
                    (1.0..=6.0).contains(&v).then_some((v, w.parse::<u8>().ok()?.clamp(1, 3)))
                })
                .take(200)
                .collect();
            Some(Subject { name: name.to_string(), grades })
        })
        .take(60)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_parse_and_average() {
        assert_eq!(parse_grade("2"), Some(2.0));
        assert_eq!(parse_grade("2,5"), Some(2.5));
        assert_eq!(parse_grade("2+"), Some(1.7));
        assert_eq!(parse_grade("2-"), Some(2.3));
        assert_eq!(parse_grade("7"), None);
        assert_eq!(parse_grade("abc"), None);
        let s = Subject { name: "Mathe".into(), grades: vec![(1.0, 2), (4.0, 1)] };
        assert!((s.average().unwrap() - 2.0).abs() < 1e-6);
        let e = Subject { name: "Leer".into(), grades: vec![] };
        assert_eq!(overall(&[s.clone(), e.clone()]), Some(2.0));
        let enc = encode_subjects(&[s.clone(), e]);
        assert_eq!(decode_subjects(&enc)[0], s);
        let ex = vec![Exam { day: 20_000, name: "Zwischenprüfung".into() }];
        assert_eq!(decode_exams(&encode_exams(&ex)), ex);
        assert_eq!(fmt_grade(2.3), "2,3");
    }
}
