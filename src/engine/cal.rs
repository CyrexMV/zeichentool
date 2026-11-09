//! Kalender-Rechnungen (gregorianisch) für den Jahresplaner und die Lernstatistik.

/// Tage seit 1970-01-01 für ein Datum (Monat 1–12).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Datum (Jahr, Monat 1–12, Tag) zu Tagen seit 1970-01-01.
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// Wochentag: 0 = Montag … 6 = Sonntag.
pub fn weekday(z: i64) -> u32 {
    (z + 3).rem_euclid(7) as u32
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// ISO-Kalenderwoche.
pub fn iso_week(z: i64) -> u32 {
    let thursday = z - weekday(z) as i64 + 3;
    let (ty, _, _) = civil_from_days(thursday);
    ((thursday - days_from_civil(ty, 1, 1)) / 7 + 1) as u32
}

/// Montag der Woche, in der der 1. Januar liegt.
pub fn first_monday(year: i32) -> i64 {
    let j = days_from_civil(year, 1, 1);
    j - weekday(j) as i64
}

/// Anzahl der Wochenseiten eines Jahres (Montag–Sonntag, vom 1.1. bis 31.12.).
pub fn weeks_in_year(year: i32) -> u32 {
    let last = days_from_civil(year, 12, 31);
    ((last - first_monday(year)) / 7 + 1) as u32
}

/// Index der Wochenseite, in der ein Tag des Jahres liegt.
pub fn week_index(year: i32, z: i64) -> u32 {
    ((z - first_monday(year)) / 7).max(0) as u32
}

pub const MONTHS: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];
pub const MONTHS_SHORT: [&str; 12] = ["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"];

/// Tag (seit 1970) aus Millisekunden-Zeitstempel in lokaler Zeit (Versatz in Minuten, wie JS getTimezoneOffset).
pub fn day_of_ms(ms: f64, tz_offset_min: f64) -> i64 {
    ((ms - tz_offset_min * 60_000.0) / 86_400_000.0).floor() as i64
}

/// Ein Lerntag: (Tag seit 1970, beantwortet, gewusst).
pub type Day = (i64, u32, u32);

pub fn parse_stats(s: &str) -> Vec<Day> {
    let mut v: Vec<Day> = s
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Some((f.first()?.parse().ok()?, f.get(1)?.parse().ok()?, f.get(2)?.parse().ok()?))
        })
        .collect();
    v.sort_by_key(|d| d.0);
    v
}

/// (aktuelle Serie, längste Serie). Die Serie läuft weiter, wenn heute noch nicht gelernt wurde.
pub fn streaks(days: &[Day], today: i64) -> (u32, u32) {
    let active: Vec<i64> = days.iter().filter(|d| d.1 > 0).map(|d| d.0).collect();
    let (mut best, mut run, mut prev) = (0, 0, i64::MIN);
    for &d in &active {
        run = if d == prev + 1 { run + 1 } else { 1 };
        best = best.max(run);
        prev = d;
    }
    let mut cur = 0;
    let mut day = if active.contains(&today) { today } else { today - 1 };
    while active.contains(&day) {
        cur += 1;
        day -= 1;
    }
    (cur, best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(days_from_civil(2027, 2, 28) + 1), (2027, 3, 1));
        assert_eq!(civil_from_days(days_from_civil(2028, 2, 28) + 1), (2028, 2, 29));
        assert_eq!(weekday(days_from_civil(2026, 11, 9)), 0, "9.11.2026 ist ein Montag");
        assert_eq!(iso_week(days_from_civil(2026, 1, 1)), 1);
        assert_eq!(iso_week(days_from_civil(2027, 1, 1)), 53);
        assert_eq!(iso_week(days_from_civil(2026, 12, 31)), 53);
        assert!((53..=54).contains(&weeks_in_year(2027)));
        assert_eq!(week_index(2027, days_from_civil(2027, 1, 1)), 0);
    }

    #[test]
    fn streak_counts() {
        let d = vec![(10, 3, 2), (11, 1, 1), (12, 5, 5), (20, 1, 0), (21, 2, 2)];
        assert_eq!(streaks(&d, 21), (2, 3));
        assert_eq!(streaks(&d, 22), (2, 3), "heute noch nicht gelernt – Serie bleibt");
        assert_eq!(streaks(&d, 23), (0, 3));
        assert_eq!(parse_stats("5\t2\t1\n3\t1\t1"), vec![(3, 1, 1), (5, 2, 1)]);
    }
}
