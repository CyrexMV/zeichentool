//! Formelsammlung Berufskraftfahrer: vier fertige Seiten (Fahrphysik, Technik & Kosten,
//! Laden & Ladungssicherung, Recht). Alles als normale Elemente (Text, Linien, Flächen) –
//! man kann darin markieren, ergänzen, radieren und Lernkarten daraus machen.

use super::*;
use crate::engine::history::Action;
use crate::engine::{Content, Item, Page, PenKind, Pt, Stroke, TextBox, Xf};

/// Ein Baustein der Formelsammlung.
enum B {
    /// Abschnitt mit Farbe
    H(&'static str, [u8; 3]),
    /// Formel: Bezeichnung, Formel („/“ = Bruch)
    F(&'static str, &'static str),
    /// Stichwort: Wert (zweispaltig)
    K(&'static str, &'static str),
    /// Hinweistext über die ganze Breite
    T(&'static str),
}

const BLUE: [u8; 3] = [30, 90, 230];
const ORANGE: [u8; 3] = [232, 115, 12];
const GREEN: [u8; 3] = [23, 148, 90];
const PURPLE: [u8; 3] = [124, 58, 237];
const RED: [u8; 3] = [224, 50, 42];

const PAGES: &[(&str, &[B])] = &[
    (
        "Fahrphysik",
        &[
            B::H("Geschwindigkeit, Weg, Zeit", BLUE),
            B::F("Geschwindigkeit", "v = s/t"),
            B::F("Weg", "s = v · t"),
            B::F("Zeit", "t = s/v"),
            B::F("Durchschnittsgeschwindigkeit", "vØ = (Gesamtstrecke)/(Gesamtzeit)"),
            B::K("Umrechnung", "km/h : 3,6 = m/s   ·   m/s × 3,6 = km/h   (72 km/h = 20 m/s)"),
            B::H("Anhalteweg – Faustformeln (v in km/h, Ergebnis in m)", ORANGE),
            B::F("Reaktionsweg (1 s)", "sR = v/10 · 3"),
            B::F("Bremsweg normal", "sB = v/10 · v/10"),
            B::F("Bremsweg Gefahrenbremsung", "sB = v/10 · v/10 · 1/2"),
            B::F("Anhalteweg", "sA = sR + sB"),
            B::T("Doppelte Geschwindigkeit → vierfacher Bremsweg. Nässe, Glätte, Beladung und Reifen verlängern den Bremsweg."),
            B::H("Rechenbeispiel: 80 km/h", PURPLE),
            B::K("Reaktionsweg", "80/10 · 3 = 8 · 3 = 24 m"),
            B::K("Bremsweg normal", "8 · 8 = 64 m   →   Anhalteweg 24 m + 64 m = 88 m"),
            B::K("Gefahrenbremsung", "64 m : 2 = 32 m   →   Anhalteweg 24 m + 32 m = 56 m"),
            B::K("Abstand", "halber Tacho = 40 m"),
            B::H("Sicherheitsabstand", GREEN),
            B::F("Faustregel „halber Tacho“", "Abstand = v/2   [m]"),
            B::K("Lkw über 3,5 t", "bei mehr als 50 km/h auf Autobahnen mind. 50 m Abstand (§ 4 StVO)"),
            B::K("Zeitabstand", "mindestens 2 Sekunden, bei Nässe/Dunkelheit mehr"),
        ],
    ),
    (
        "Physik & Technik",
        &[
            B::H("Bremsweg – physikalisch (v in m/s)", PURPLE),
            B::F("Reaktionsweg", "sR = v · tR"),
            B::F("Bremsweg", "sB = v²/(2 · a)"),
            B::K("a", "Bremsverzögerung in m/s² (trockene Straße, Lkw ca. 5 m/s²)"),
            B::H("Kräfte & Energie", RED),
            B::F("Kraft", "F = m · a"),
            B::F("Gewichtskraft", "FG = m · g      (g = 9,81 m/s² ≈ 10 m/s²)"),
            B::F("Fliehkraft", "F = m · v²/r"),
            B::F("Bewegungsenergie", "E = 1/2 · m · v²"),
            B::T("Doppelte Geschwindigkeit → vierfache Fliehkraft und Bewegungsenergie (geht mit v² ein)."),
            B::H("Leistung & Motor", BLUE),
            B::K("Umrechnung", "1 PS = 0,735 kW   ·   1 kW = 1,36 PS"),
            B::F("Arbeit", "W = F · s"),
            B::F("Leistung", "P = W/t"),
            B::F("Drehmoment", "M = F · r"),
            B::F("Motorleistung (kW)", "P = (M · n)/9550"),
            B::K("", "M in Nm, n = Drehzahl in 1/min"),
            B::F("Übersetzung", "i = (Drehzahl Antrieb)/(Drehzahl Abtrieb)"),
            B::F("Steigung (%)", "(Höhenunterschied)/(waagerechte Strecke) · 100"),
            B::T("Wirtschaftlich fahren: im grünen Drehzahlbereich bleiben (größtes Drehmoment), vorausschauend rollen lassen."),
        ],
    ),
    (
        "Kosten & Fahrzeug",
        &[
            B::H("Verbrauch & Kosten", ORANGE),
            B::F("Ø Verbrauch (l/100 km)", "(getankte Liter · 100)/(gefahrene km)"),
            B::F("Kraftstoff für Strecke", "(Strecke · Verbrauch)/100"),
            B::F("Kraftstoffkosten", "Liter · Preis je Liter"),
            B::F("Kosten je km", "(Gesamtkosten)/(gefahrene km)"),
            B::F("Kosten je Stunde", "(Gesamtkosten)/(Einsatzstunden)"),
            B::K("AdBlue", "Verbrauch ca. 3–5 % des Dieselverbrauchs"),
            B::K("Kostenarten", "fix (Abschreibung, Steuer, Versicherung, Lohn) · variabel (Kraftstoff, Reifen, Wartung, Maut)"),
            B::H("Reifen & Bremsen", GREEN),
            B::K("Profiltiefe", "gesetzlich mindestens 1,6 mm"),
            B::K("Reifendruck", "im kalten Zustand prüfen – falscher Druck erhöht Verschleiß und Verbrauch"),
            B::K("Dauerbremse", "Retarder/Motorbremse im Gefälle nutzen – Betriebsbremse schonen (Fading)"),
            B::K("Druckluftbremse", "vor Fahrtantritt Druck, Dichtheit und Kupplungsköpfe prüfen"),
        ],
    ),
    (
        "Laden & Ladungssicherung",
        &[
            B::H("Gewichte & Volumen", BLUE),
            B::F("Nutzlast", "Nutzlast = zGG − Leergewicht"),
            B::K("zGG", "zulässiges Gesamtgewicht"),
            B::F("Ladevolumen", "V = L · B · H   [m³]"),
            B::F("Dichte", "ρ = m/V"),
            B::F("Volumengewicht (kg)", "(L · B · H in cm)/6000"),
            B::K("", "KEP/Luftfracht – je nach Dienstleister auch Teiler 5000"),
            B::H("Paletten & Lademeter", ORANGE),
            B::K("Europalette", "1,20 m × 0,80 m × 0,144 m, Nutzlast bis ca. 1.500 kg"),
            B::F("Paletten längs (3 nebeneinander)", "(Ladeflächenlänge)/(1,2 m) · 3"),
            B::F("Paletten quer (2 nebeneinander)", "(Ladeflächenlänge)/(0,8 m) · 2"),
            B::K("Sattelauflieger 13,6 m", "längs 11 × 3 = 33   ·   quer 17 × 2 = 34 Europaletten"),
            B::F("Lademeter (LDM)", "(Länge · Breite)/(2,4 m)"),
            B::K("", "1 Europalette = 0,4 Lademeter"),
            B::H("Ladungssicherung", RED),
            B::K("Massenkräfte", "nach vorn 0,8 · FG   ·   nach hinten 0,5 · FG   ·   zur Seite 0,5 · FG"),
            B::F("Gewichtskraft", "FG = m · g"),
            B::F("nötige Sicherungskraft", "F = (c − μ) · m · g"),
            B::F("Vorspannkraft Niederzurren", "FV = ((c − μ) · m · g)/(μ · sin α)"),
            B::K("Zeichen", "c = Beiwert (0,8 / 0,5) · μ = Reibbeiwert · α = Zurrwinkel zur Ladefläche"),
            B::K("Reibbeiwerte μ", "Antirutschmatte ca. 0,6 · Holz auf Holz ca. 0,2–0,5 (Richtwerte)"),
            B::K("Zurrgurt", "LC = zulässige Zugkraft · STF = Vorspannkraft (in daN, 1 daN ≈ 1 kg)"),
            B::T("Je steiler der Zurrwinkel (90°), desto besser wirkt das Niederzurren. Formschlüssig laden: an die Stirnwand, Lücken füllen."),
        ],
    ),
    (
        "Recht & Vorschriften",
        &[
            B::H("Lenk- und Ruhezeiten (VO (EG) Nr. 561/2006)", BLUE),
            B::K("Tageslenkzeit", "9 h, zweimal pro Woche 10 h"),
            B::K("Wochenlenkzeit", "höchstens 56 h"),
            B::K("Doppelwoche", "höchstens 90 h"),
            B::K("Fahrtunterbrechung", "nach 4,5 h Lenkzeit 45 min – teilbar in 15 min + 30 min"),
            B::K("Tägliche Ruhezeit", "11 h (teilbar 3 h + 9 h); bis zu 3× zwischen zwei Wochenruhezeiten auf 9 h verkürzbar"),
            B::K("Wöchentliche Ruhezeit", "45 h, verkürzt mind. 24 h – Ausgleich bis Ende der 3. Folgewoche"),
            B::K("Spätestens", "nach sechs 24-Stunden-Zeiträumen beginnt die Wochenruhezeit"),
            B::K("Mehrfahrerbetrieb", "9 h Ruhezeit innerhalb von 30 h"),
            B::H("Arbeitszeit, Fahrerkarte, Weiterbildung", ORANGE),
            B::K("Arbeitszeit", "im Schnitt höchstens 48 h/Woche (4 Monate), einzelne Woche bis 60 h"),
            B::K("Ruhepausen (ArbZG)", "6–9 h Arbeit: 30 min · mehr als 9 h: 45 min"),
            B::K("Fahrerkarte", "Aufzeichnungen des laufenden Tages und der vorausgehenden 56 Tage vorlegen können"),
            B::K("Weiterbildung", "alle 5 Jahre 35 Stunden (BKrFQG) – Schlüsselzahl 95 im Führerschein"),
            B::H("Maße & Gewichte (StVZO)", GREEN),
            B::K("Breite", "2,55 m (Kühlaufbau 2,60 m)"),
            B::K("Höhe", "4,00 m"),
            B::K("Länge", "Lkw 12 m · Sattelkraftfahrzeug 16,50 m · Gliederzug 18,75 m"),
            B::K("Gesamtgewicht", "Sattel-/Gliederzug 40 t (im kombinierten Verkehr 44 t)"),
            B::K("Achslast", "Einzelachse 10 t · Antriebsachse 11,5 t"),
            B::H("Geschwindigkeiten (StVO) & Gefahrgut", RED),
            B::K("Lkw 3,5 t – 7,5 t", "außerorts 80 km/h · Autobahn 80 km/h"),
            B::K("Lkw über 7,5 t", "außerorts 60 km/h · Autobahn 80 km/h"),
            B::K("innerorts", "50 km/h"),
            B::K("Warntafel (ADR)", "orange, oben Gefahrnummer · unten UN-Nummer"),
            B::K("Gefahrgutklassen", "1 explosiv · 2 Gase · 3 entzündbar flüssig · 4.1–4.3 entzündbar fest · 5.1 entzündend (oxidierend) · 5.2 organische Peroxide · 6.1 giftig · 6.2 ansteckungsgefährlich · 7 radioaktiv · 8 ätzend · 9 verschiedene"),
        ],
    ),
];

const W: f32 = 595.0;
const H: f32 = 842.0;
const LEFT: f32 = 40.0;
const COL: f32 = 214.0;
const RIGHT: f32 = 555.0;
const DARK: [u8; 4] = [28, 30, 44, 255];
const MUTED: [u8; 4] = [96, 100, 118, 255];

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Vec<Pt> {
    // abgerundetes Rechteck als Polygon
    let mut pts = Vec::new();
    let corners = [(x1 - r, y0 + r, -90.0f32), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, 90.0), (x0 + r, y0 + r, 180.0)];
    for (cx, cy, a0) in corners {
        for k in 0..=4 {
            let a = (a0 + k as f32 * 22.5).to_radians();
            pts.push(Pt::new(cx + r * a.cos(), cy + r * a.sin(), 1.0));
        }
    }
    pts
}

fn fill(pts: Vec<Pt>, c: [u8; 4]) -> Content {
    Content::Stroke(Stroke { pen: PenKind::Fill, color: c, width: 0.0, pts })
}

/// Textbreite messen (für Umbruch und Zeilenhöhe).
struct Measure(web_sys::CanvasRenderingContext2d);

impl Measure {
    fn width(&self, t: &str, size: f32, bold: bool) -> f32 {
        let tb = TextBox { x: 0.0, y: 0.0, w: 0.0, size, color: [0; 4], bold, text: String::new() };
        self.0.set_font(&super::render::text_font(&tb));
        self.0.measure_text(t).map(|m| m.width() as f32).unwrap_or(t.chars().count() as f32 * size * 0.55)
    }

    /// Anzahl Zeilen bei Umbruch auf `w`.
    fn lines(&self, t: &str, size: f32, bold: bool, w: f32) -> usize {
        let mut n = 1;
        let mut line = String::new();
        for word in t.split(' ') {
            let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if self.width(&cand, size, bold) > w && !line.is_empty() {
                n += 1;
                line = word.to_string();
            } else {
                line = cand;
            }
        }
        n
    }
}

fn text(x: f32, y: f32, w: f32, size: f32, color: [u8; 4], bold: bool, t: &str) -> Content {
    Content::Text(TextBox { x, y, w, size, color, bold, text: t.to_string() })
}

/// Unterer Rand, ab dem eine neue Seite beginnt.
const BOTTOM: f32 = H - 52.0;

/// Abstand für einen Block (Höhe, die er auf der Seite braucht).
fn block_height(b: &B, m: &Measure) -> f32 {
    match b {
        B::H(..) => 38.0 + 30.0, // Kopf + mindestens eine Zeile, damit er nicht allein unten steht
        B::F(_, f) => {
            let (_, _, fh) = super::fraction::layout(f, 12.5);
            fh.max(12.0 * 1.35) + 3.0
        }
        B::K(k, v) => m.lines(v, 10.5, false, RIGHT - COL - 8.0).max(m.lines(k, 10.5, true, COL - LEFT - 14.0)) as f32 * 10.5 * 1.32 + 5.0,
        B::T(t) => m.lines(t, 10.5, false, RIGHT - LEFT - 16.0) as f32 * 10.5 * 1.32 + 5.0,
    }
}

/// Alle Seiten im A4-Raster (595 × 842 pt); Abschnitte, die nicht mehr passen, beginnen auf einer neuen Seite.
fn build_pages(m: &Measure) -> Vec<(String, Vec<Content>)> {
    let mut pages: Vec<(String, Vec<Content>)> = Vec::new();
    for (title, blocks) in PAGES {
        let mut out: Vec<Content> = Vec::new();
        let mut y = 96.0f32;
        let mut panel: Option<(f32, [u8; 3])> = None;
        let mut part = 0;
        let close_panel = |out: &mut Vec<Content>, panel: &mut Option<(f32, [u8; 3])>, y: f32| {
            if let Some((top, c)) = panel.take() {
                out.push(fill(rect(LEFT, top, RIGHT, y + 4.0, 8.0), [c[0], c[1], c[2], 22]));
            }
        };
        for (bi, b) in blocks.iter().enumerate() {
            // ganzer Abschnitt passt nicht mehr, auf einer leeren Seite aber schon → dort beginnen
            let section_break = matches!(b, B::H(..)) && y > 140.0 && {
                let end = blocks[bi + 1..].iter().position(|x| matches!(x, B::H(..))).map(|p| bi + 1 + p).unwrap_or(blocks.len());
                let sh: f32 = blocks[bi..end].iter().map(|x| block_height(x, m)).sum::<f32>() - 30.0;
                y + sh > BOTTOM && 96.0 + sh <= BOTTOM
            };
            if section_break || y + block_height(b, m) > BOTTOM {
                // Seitenumbruch: Abschnitt (mit Farbe) auf der neuen Seite fortsetzen
                let color = panel.map(|p| p.1);
                close_panel(&mut out, &mut panel, y);
                let name = if part == 0 { title.to_string() } else { format!("{title} (Fortsetzung)") };
                pages.push((name, std::mem::take(&mut out)));
                part += 1;
                y = 96.0;
                if let (Some(c), false) = (color, matches!(b, B::H(..))) {
                    panel = Some((y - 6.0, c));
                }
            }
            match b {
                B::H(t, c) => {
                    close_panel(&mut out, &mut panel, y);
                    y += 10.0;
                    out.push(fill(rect(LEFT, y, RIGHT, y + 22.0, 8.0), [c[0], c[1], c[2], 255]));
                    out.push(text(LEFT + 10.0, y + 4.0, RIGHT - LEFT - 20.0, 12.0, [255, 255, 255, 255], true, t));
                    panel = Some((y + 18.0, *c));
                    y += 28.0;
                }
                B::F(label, f) => {
                    let (fc, _fw, fh) = super::fraction::formula_contents(f, 12.5, COL, y, DARK);
                    let lh = 12.0 * 1.35;
                    let row = fh.max(lh);
                    out.push(text(LEFT + 8.0, y + (row - lh) * 0.5, COL - LEFT - 14.0, 10.5, MUTED, true, label));
                    // Formel senkrecht mittig in der Zeile
                    let dy = (row - fh) * 0.5;
                    out.extend(fc.into_iter().map(|c| match c {
                        Content::Text(mut t) => {
                            t.y += dy;
                            Content::Text(t)
                        }
                        Content::Stroke(mut s) => {
                            for p in s.pts.iter_mut() {
                                p.y += dy;
                            }
                            Content::Stroke(s)
                        }
                        other => other,
                    }));
                    y += row + 3.0;
                }
                B::K(k, v) => {
                    let vw = RIGHT - COL - 8.0;
                    if !k.is_empty() {
                        out.push(text(LEFT + 8.0, y, COL - LEFT - 14.0, 10.5, MUTED, true, k));
                    }
                    out.push(text(COL, y, vw, 10.5, DARK, false, v));
                    y += block_height(b, m);
                }
                B::T(t) => {
                    out.push(text(LEFT + 8.0, y, RIGHT - LEFT - 16.0, 10.5, DARK, false, t));
                    y += block_height(b, m);
                }
            }
        }
        close_panel(&mut out, &mut panel, y);
        let name = if part == 0 { title.to_string() } else { format!("{title} (Fortsetzung)") };
        pages.push((name, out));
    }
    // Kopf und Fuß mit der endgültigen Seitenzahl
    let n = pages.len();
    for (k, (name, out)) in pages.iter_mut().enumerate() {
        let mut head = vec![
            text(LEFT, 30.0, 420.0, 21.0, DARK, true, "Formelsammlung Berufskraftfahrer"),
            text(LEFT, 58.0, 420.0, 12.0, MUTED, false, &format!("{} · Seite {} von {}", name, k + 1, n)),
            fill(rect(LEFT, 80.0, RIGHT, 82.5, 1.0), [109, 92, 255, 255]),
        ];
        head.append(out);
        head.push(text(LEFT, H - 34.0, RIGHT - LEFT, 8.5, MUTED, false, "Angaben ohne Gewähr – maßgeblich sind die jeweils aktuellen Vorschriften und dein Ausbildungsbetrieb/deine Berufsschule."));
        *out = head;
    }
    pages
}

impl App {
    /// Formelsammlung als neue Seiten hinter der aktuellen Seite einfügen.
    pub fn insert_bkf(&mut self) {
        self.close_popovers();
        let (_, ctx) = super::editor::new_canvas();
        let m = Measure(ctx);
        let Some(ed) = &mut self.ed else { return };
        let at = ed.cur_page + 1;
        let (fw, fh) = (ed.nb.format.w, ed.nb.format.h);
        // auf das Seitenformat des Notizbuchs einpassen (Raster ist A4)
        let s = (fw / W).min(fh / H);
        let ox = (fw - W * s) * 0.5;
        let xf = Xf { a: s, b: 0.0, c: 0.0, d: s, e: ox, f: 0.0 };
        let mut group = Vec::new();
        let pages = build_pages(&m);
        let count = pages.len();
        for (k, (title, contents)) in pages.into_iter().enumerate() {
            let id = ed.nb.alloc_id();
            let mut page = Page::new(id, Default::default(), Color::WHITE);
            page.title = format!("BKF-Formeln: {title}");
            let mut items = Vec::new();
            for (n, c) in contents.into_iter().enumerate() {
                let iid = ed.nb.alloc_id();
                items.push(Item { id: iid, order: n as f64 + 1.0, content: c, pinned: false }.transformed(&xf));
            }
            page.insert(items);
            ed.nb.pages.insert(at + k, page.clone());
            group.push(Action::AddPage { index: at + k, page });
        }
        ed.push(Action::Group(group));
        ed.goto_page(at);
        self.toast(&format!("Formelsammlung Berufskraftfahrer eingefügt – {count} Seiten, im Inhaltsverzeichnis unter „BKF-Formeln“"));
        self.after_pages_change();
    }
}
