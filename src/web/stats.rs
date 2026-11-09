//! Lernstatistik: beantwortete Karten pro Tag, Serie (Tage am Stück), Trefferquote.

use super::*;
use crate::engine::cal;

pub use crate::engine::cal::{parse_stats, streaks, Day};

pub fn today() -> i64 {
    let tz = js_sys::Date::new_0().get_timezone_offset();
    cal::day_of_ms(wall_clock(), tz)
}

impl App {
    pub fn record_answer(&mut self, known: bool) {
        let t = today();
        match self.stats.iter_mut().find(|d| d.0 == t) {
            Some(d) => {
                d.1 += 1;
                d.2 += known as u32;
            }
            None => self.stats.push((t, 1, known as u32)),
        }
        self.persist_stats();
    }

    pub fn persist_stats(&self) {
        let s: Vec<String> = self.stats.iter().map(|d| format!("{}\t{}\t{}", d.0, d.1, d.2)).collect();
        super::storage::put_meta(self, "stats", &s.join("\n"));
    }

    /// Kurzinfo „🔥 5 Tage · heute 12 Karten“ (leer, wenn noch nie gelernt).
    pub fn streak_line(&self) -> String {
        let t = today();
        let (cur, _) = streaks(&self.stats, t);
        let n_today = self.stats.iter().find(|d| d.0 == t).map(|d| d.1).unwrap_or(0);
        if self.stats.is_empty() {
            return String::new();
        }
        format!(
            "<span class=\"streak{}\"><svg><use href=\"#i-flame\"/></svg>{} {}</span><span class=\"muted\">heute {} {}</span>",
            if cur > 0 { " on" } else { "" },
            cur,
            if cur == 1 { "Tag" } else { "Tage" },
            n_today,
            if n_today == 1 { "Karte" } else { "Karten" }
        )
    }

    pub fn open_stats(&mut self) {
        self.close_popovers();
        let t = today();
        let (cur, best) = streaks(&self.stats, t);
        let total: u32 = self.stats.iter().map(|d| d.1).sum();
        let right: u32 = self.stats.iter().map(|d| d.2).sum();
        let n_today = self.stats.iter().find(|d| d.0 == t).map(|d| d.1).unwrap_or(0);
        let days_learned = self.stats.iter().filter(|d| d.1 > 0).count();
        let acc = (right * 100).checked_div(total).unwrap_or(0);
        let mut h = format!(
            "<div class=\"stats-hero\"><div class=\"flame{}\"><svg><use href=\"#i-flame\"/></svg></div><div><b>{}</b><span>{} am Stück gelernt</span></div></div>\
             <div class=\"stats-grid\">\
               <div><b>{}</b><small>Heute</small></div><div><b>{}</b><small>Rekord-Serie</small></div>\
               <div><b>{}</b><small>Karten gesamt</small></div><div><b>{}%</b><small>gewusst</small></div>\
             </div>",
            if cur > 0 { " on" } else { "" },
            cur,
            if cur == 1 { "Tag" } else { "Tage" },
            n_today,
            best,
            total,
            acc
        );
        // Balken der letzten 30 Tage
        let max = (0..30).map(|k| self.stats.iter().find(|d| d.0 == t - k).map(|d| d.1).unwrap_or(0)).max().unwrap_or(0).max(1);
        h.push_str("<h4>Letzte 30 Tage</h4><div class=\"stats-bars\">");
        for k in (0..30).rev() {
            let day = t - k;
            let (n, r) = self.stats.iter().find(|d| d.0 == day).map(|d| (d.1, d.2)).unwrap_or((0, 0));
            let (_, mo, dd) = cal::civil_from_days(day);
            let hgt = n as f64 / max as f64 * 100.0;
            let ok = if n == 0 { 0.0 } else { r as f64 / n as f64 * 100.0 };
            h.push_str(&format!(
                "<div class=\"bar{}{}\" title=\"{}.{}.: {} Karten, {} gewusst\"><span style=\"height:{:.0}%\"><i style=\"height:{:.0}%\"></i></span></div>",
                if k == 0 { " today" } else { "" },
                if n == 0 { " empty" } else { "" },
                dd,
                mo,
                n,
                r,
                hgt,
                ok
            ));
        }
        h.push_str("</div><div class=\"stats-legend\"><span><i class=\"ok\"></i>gewusst</span><span><i></i>nochmal</span><span class=\"muted\">");
        h.push_str(&format!("{} Lerntage</span></div>", days_learned));
        // Sets
        let now = wall_clock();
        let mut decks: Vec<&crate::engine::format::NoteMeta> = self.library.iter().filter(|m| m.visible() && m.cards > 0).collect();
        decks.sort_by_key(|m| std::cmp::Reverse(m.progress));
        if !decks.is_empty() {
            h.push_str("<h4>Deine Sets</h4><div class=\"stats-decks\">");
            for m in decks {
                let due = if self.settings.due_mode { m.due_count(now) } else { 0 };
                h.push_str(&format!(
                    "<div class=\"sd\"><span class=\"mini\" style=\"{}\"></span><div><b>{}</b><small>{} Karten · {} gelernt{}</small><div class=\"deck-prog\"><span style=\"width:{}%\"></span></div></div></div>",
                    super::library::cover_css(m.cover),
                    esc(&m.title),
                    m.cards,
                    m.learned,
                    if due > 0 { format!(" · <b class=\"due\">{} fällig</b>", due) } else { String::new() },
                    m.progress.min(100)
                ));
            }
            h.push_str("</div>");
        }
        el("stats-body").set_inner_html(&h);
        self.open_modal("modal-stats");
    }
}

