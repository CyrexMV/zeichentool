//! Prüfungs-Countdown (Startbildschirm) und Notenübersicht.

use super::*;
use crate::engine::cal;
use crate::engine::school::{fmt_grade, overall, parse_grade, Exam, Subject};

const WEEKDAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

fn grade_class(v: f32) -> &'static str {
    if v <= 2.5 {
        "good"
    } else if v <= 3.5 {
        "mid"
    } else {
        "bad"
    }
}

fn date_label(day: i64) -> String {
    let (y, m, d) = cal::civil_from_days(day);
    format!("{}, {}. {} {}", WEEKDAYS[cal::weekday(day) as usize % 7], d, cal::MONTHS[(m as usize + 11) % 12], y)
}

fn input_value(id: &str) -> String {
    input_el(id).value()
}

impl App {
    // ------------------------------------------------------------------
    // Startbildschirm-Leiste
    // ------------------------------------------------------------------

    pub fn render_school(&self, visible: bool) {
        show("lib-school", visible);
        if !visible {
            return;
        }
        let today = super::stats::today();
        let mut exams: Vec<(usize, &Exam)> = self.exams.iter().enumerate().filter(|(_, e)| e.day >= today).collect();
        exams.sort_by_key(|(_, e)| e.day);
        let mut h = String::from("<div class=\"school-row\">");
        for (i, e) in exams.iter().take(6) {
            let left = e.day - today;
            let (num, unit) = match left {
                0 => ("Heute".to_string(), "".to_string()),
                1 => ("Morgen".to_string(), "".to_string()),
                n => (n.to_string(), "Tage".to_string()),
            };
            let cls = if left <= 7 { "soon" } else if left <= 30 { "mid" } else { "far" };
            h.push_str(&format!(
                "<button class=\"exam-card {cls}\" data-act=\"exam-edit\" data-arg=\"{i}\" title=\"Bearbeiten\"><span class=\"exam-days{small}\"><b>{num}</b><small>{unit}</small></span><span class=\"exam-info\"><small>noch bis</small><b>{name}</b><small>{date}</small></span></button>",
                small = if left <= 1 { " word" } else { "" },
                name = esc(&e.name),
                date = date_label(e.day)
            ));
        }
        h.push_str("<button class=\"exam-add\" data-act=\"exam-new\"><svg><use href=\"#i-plus\"/></svg><span><b>Prüfung</b><small>Countdown anlegen</small></span></button>");
        let avg = overall(&self.subjects);
        h.push_str(&format!(
            "<button class=\"grades-card\" data-act=\"grades-open\"><span class=\"grades-ico\"><svg><use href=\"#i-grades\"/></svg></span><span><b>Noten</b><small>{}</small></span>{}</button>",
            if self.subjects.is_empty() { "Fächer &amp; Durchschnitt".to_string() } else { format!("{} {}", self.subjects.len(), if self.subjects.len() == 1 { "Fach" } else { "Fächer" }) },
            match avg {
                Some(v) => format!("<span class=\"grades-avg {}\">Ø {}</span>", grade_class(v), fmt_grade(v)),
                None => String::new(),
            }
        ));
        h.push_str("</div>");
        el("lib-school").set_inner_html(&h);
    }

    // ------------------------------------------------------------------
    // Prüfung anlegen / bearbeiten
    // ------------------------------------------------------------------

    pub fn exam_action(&mut self, act: &str, arg: &str) {
        match act {
            "exam-new" | "exam-edit" => {
                let idx = if act == "exam-edit" { arg.parse::<usize>().ok().filter(|i| *i < self.exams.len()) } else { None };
                self.exam_edit = idx;
                let (name, day) = match idx {
                    Some(i) => (self.exams[i].name.clone(), self.exams[i].day),
                    None => (String::new(), super::stats::today() + 30),
                };
                let (y, m, d) = cal::civil_from_days(day);
                input_el("exam-name").set_value(&name);
                input_el("exam-date").set_value(&format!("{:04}-{:02}-{:02}", y, m, d));
                set_text("exam-title", if idx.is_some() { "Prüfung bearbeiten" } else { "Neue Prüfung" });
                show("exam-delete", idx.is_some());
                self.open_modal("modal-exam");
                if idx.is_none() {
                    let _ = input_el("exam-name").focus();
                }
            }
            "exam-save" => {
                let name = input_value("exam-name").trim().to_string();
                let date = input_value("exam-date");
                let parts: Vec<&str> = date.split('-').collect();
                let day = match parts.as_slice() {
                    [y, m, d] => match (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>()) {
                        (Ok(y), Ok(m), Ok(d)) if (1..=12).contains(&m) && (1..=31).contains(&d) => Some(cal::days_from_civil(y, m, d)),
                        _ => None,
                    },
                    _ => None,
                };
                let Some(day) = day else {
                    self.toast("Bitte ein Datum wählen");
                    return;
                };
                let name = if name.is_empty() { "Prüfung".to_string() } else { name };
                match self.exam_edit.take() {
                    Some(i) if i < self.exams.len() => self.exams[i] = Exam { day, name },
                    _ => self.exams.push(Exam { day, name }),
                }
                self.close_modals();
                self.save_settings();
                self.render_library();
            }
            "exam-delete" => {
                if let Some(i) = self.exam_edit.take() {
                    if i < self.exams.len() {
                        self.exams.remove(i);
                    }
                }
                self.close_modals();
                self.save_settings();
                self.render_library();
                self.toast("Prüfung entfernt");
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Notenübersicht
    // ------------------------------------------------------------------

    pub fn open_grades(&mut self) {
        self.open_modal("modal-grades");
        self.render_grades();
    }

    pub fn render_grades(&self) {
        let avg = overall(&self.subjects);
        set_text("grades-total", &avg.map(|v| format!("Ø {}", fmt_grade(v))).unwrap_or_else(|| "–".into()));
        let tot = el("grades-total");
        for c in ["good", "mid", "bad"] {
            set_class(&tot, c, avg.map(grade_class) == Some(c));
        }
        for (w, id) in [(1u8, "gw-1"), (2, "gw-2")] {
            set_class(&el(id), "active", self.grade_weight == w);
        }
        let mut h = String::new();
        if self.subjects.is_empty() {
            h.push_str("<p class=\"muted grades-empty\">Noch keine Fächer – unten ein Fach anlegen, z. B. „Mathe“ oder „Lernfeld 3“.</p>");
        }
        for (i, s) in self.subjects.iter().enumerate() {
            let a = s.average();
            h.push_str(&format!(
                "<div class=\"subj\"><div class=\"subj-head\"><b>{name}</b>{avg}<button class=\"icon-btn small\" data-act=\"subject-del\" data-arg=\"{i}\" title=\"Fach löschen\"><svg><use href=\"#i-trash\"/></svg></button></div><div class=\"subj-grades\">",
                name = esc(&s.name),
                avg = match a {
                    Some(v) => format!("<span class=\"subj-avg {}\">Ø {}</span>", grade_class(v), fmt_grade(v)),
                    None => "<span class=\"subj-avg\">–</span>".into(),
                }
            ));
            for (k, g) in s.grades.iter().enumerate() {
                h.push_str(&format!(
                    "<button class=\"grade-chip {}\" data-act=\"grade-del\" data-arg=\"{i}:{k}\" title=\"Antippen zum Entfernen\">{}{}</button>",
                    grade_class(g.0),
                    fmt_grade(g.0),
                    if g.1 > 1 { format!("<small>×{}</small>", g.1) } else { String::new() }
                ));
            }
            h.push_str(&format!(
                "<span class=\"grade-add\"><input id=\"grade-in-{i}\" placeholder=\"Note\" inputmode=\"decimal\" autocomplete=\"off\"><button data-act=\"grade-add\" data-arg=\"{i}\" title=\"Note hinzufügen\"><svg><use href=\"#i-plus\"/></svg></button></span></div></div>"
            ));
        }
        el("grades-body").set_inner_html(&h);
    }

    pub fn grades_action(&mut self, act: &str, arg: &str) {
        match act {
            "grades-open" => self.open_grades(),
            "grade-weight" => self.grade_weight = arg.parse::<u8>().unwrap_or(1).clamp(1, 2),
            "subject-add" => {
                let name = input_value("subject-name").trim().to_string();
                if name.is_empty() {
                    self.toast("Bitte einen Fachnamen eingeben");
                    return;
                }
                self.subjects.push(Subject { name, grades: Vec::new() });
                input_el("subject-name").set_value("");
                self.save_settings();
                self.render_grades();
                self.render_school(true);
                let i = self.subjects.len() - 1;
                if let Some(inp) = dom().get_element_by_id(&format!("grade-in-{i}")) {
                    let _ = inp.unchecked_into::<web_sys::HtmlElement>().focus();
                }
                return;
            }
            "grade-add" => {
                let Ok(i) = arg.parse::<usize>() else { return };
                let id = format!("grade-in-{i}");
                let raw = input_value(&id);
                let Some(v) = parse_grade(&raw) else {
                    self.toast("Note von 1 bis 6 eingeben, z. B. 2, 2,5, 2+ oder 2-");
                    return;
                };
                let w = self.grade_weight;
                if let Some(s) = self.subjects.get_mut(i) {
                    s.grades.push((v, w));
                }
                self.save_settings();
                self.render_grades();
                self.render_school(true);
                if let Some(inp) = dom().get_element_by_id(&id) {
                    let _ = inp.unchecked_into::<web_sys::HtmlElement>().focus();
                }
                return;
            }
            "grade-del" => {
                let Some((i, k)) = arg.split_once(':').and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))) else { return };
                if let Some(s) = self.subjects.get_mut(i) {
                    if k < s.grades.len() {
                        let g = s.grades.remove(k);
                        self.toast(&format!("Note {} entfernt", fmt_grade(g.0)));
                    }
                }
                self.save_settings();
            }
            "subject-del" => {
                let Ok(i) = arg.parse::<usize>() else { return };
                let Some(s) = self.subjects.get(i) else { return };
                let text = format!("„{}“ mit {} {} wird gelöscht.", esc(&s.name), s.grades.len(), if s.grades.len() == 1 { "Note" } else { "Noten" });
                self.open_dialog("", "Fach löschen?", &text, "Löschen", true, None, Pending::DeleteSubject(i));
                return;
            }
            _ => {}
        }
        self.render_grades();
        self.render_school(true);
    }

    pub fn delete_subject(&mut self, i: usize) {
        if i < self.subjects.len() {
            self.subjects.remove(i);
            self.save_settings();
        }
        self.open_grades();
    }
}
