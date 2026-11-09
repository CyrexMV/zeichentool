//! Jahresplaner: Jahresübersicht, 12 Monatsseiten und alle Wochenseiten – mit echten Daten
//! und automatischen Verweisen (Reiter, Mini-Monate, Kalenderwochen, Blättern).

use super::cal;
use super::color::Color;
use super::geom::Rect;
use super::note::{Format, Link, LinkTarget, Notebook, Page};
use super::paper::{PlanTarget, Template, TemplateKind};

pub fn year_planner(id: String, title: &str, cover: u8, format: Format, year: i32, paper: Color, now: f64) -> Notebook {
    let year16 = year.clamp(1, 9999) as u16;
    let mut nb = Notebook::new(id, title, cover, format, Template::planner(TemplateKind::YearPlan, year16, 0), paper, now);
    for m in 0..12u16 {
        let pid = nb.alloc_id();
        nb.pages.push(Page::new(pid, Template::planner(TemplateKind::MonthPlan, year16, m), paper));
    }
    for w in 0..cal::weeks_in_year(year) as u16 {
        let pid = nb.alloc_id();
        nb.pages.push(Page::new(pid, Template::planner(TemplateKind::WeekPlan, year16, w), paper));
    }
    let weeks = cal::weeks_in_year(year) as usize;
    let target_id = |nb: &Notebook, t: PlanTarget| -> Option<u32> {
        let i = match t {
            PlanTarget::Year => 0,
            PlanTarget::Month(m) => 1 + m as usize,
            PlanTarget::Week(w) if (w as usize) < weeks => 13 + w as usize,
            PlanTarget::Week(_) => return None,
        };
        nb.pages.get(i).map(|p| p.id)
    };
    let (w, h) = (format.w, format.h);
    for i in 0..nb.pages.len() {
        let hot = nb.pages[i].template.hotspots(w, h);
        let own = nb.pages[i].id;
        let links: Vec<Link> = hot
            .into_iter()
            .filter_map(|((x0, y0, x1, y1), t)| target_id(&nb, t).filter(|id| *id != own).map(|id| Link { rect: Rect::new(x0, y0, x1, y1), target: LinkTarget::Page(id), hidden: true }))
            .collect();
        nb.pages[i].links = links;
    }
    // Seitentitel fürs Inhaltsverzeichnis
    nb.pages[0].title = format!("Jahr {}", year);
    for m in 0..12 {
        nb.pages[1 + m].title = format!("{} {}", cal::MONTHS[m], year);
    }
    for k in 0..weeks {
        let monday = cal::first_monday(year) + k as i64 * 7;
        let (_, mo, d) = cal::civil_from_days(monday);
        nb.pages[13 + k].title = format!("KW {} · ab {}.{}.", cal::iso_week(monday), d, mo);
    }
    nb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_links_resolve() {
        let nb = year_planner("p".into(), "Planer 2027", 0, Format::A4_PORTRAIT, 2027, Color::WHITE, 0.0);
        assert_eq!(nb.pages.len(), 13 + cal::weeks_in_year(2027) as usize);
        // Jahresseite verweist auf alle 12 Monate
        let month_ids: Vec<u32> = nb.pages[1..13].iter().map(|p| p.id).collect();
        for id in &month_ids {
            assert!(nb.pages[0].links.iter().any(|l| l.target == LinkTarget::Page(*id)));
        }
        // jede Seite hat Verweise, alle Ziele existieren
        for p in &nb.pages {
            assert!(!p.links.is_empty());
            for l in &p.links {
                let LinkTarget::Page(t) = l.target else { panic!() };
                assert!(nb.page_index(t).is_some());
            }
        }
        assert_eq!(nb.pages[2].title, "Februar 2027");
    }
}
