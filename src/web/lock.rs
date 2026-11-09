//! PIN-Sperre für Notizbücher (mit Fingerabdruck in der Android-App) und Papierkorb (30 Tage).

use super::*;
use crate::engine::format::NoteMeta;
use crate::engine::sha::{pin_hash, pin_matches};

/// Was nach dem Entsperren passieren soll.
#[derive(Clone, Debug, PartialEq)]
pub enum Then {
    Open,
    Study,
    Card(String),
}

const TRASH_DAYS: f64 = 30.0;
const DAY_MS: f64 = 86_400_000.0;

fn pin_input(on: bool) {
    let inp = input_el("dlg-input");
    let _ = inp.set_attribute("type", if on { "password" } else { "text" });
    let _ = inp.set_attribute("inputmode", if on { "numeric" } else { "text" });
    let _ = inp.set_attribute("autocomplete", if on { "one-time-code" } else { "off" });
    inp.set_placeholder(if on { "PIN" } else { "" });
}

fn bio_available() -> bool {
    super::storage::android_bridge()
        .and_then(|b| js_sys::Reflect::get(&b, &"canAuthenticate".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()).map(|f| (b, f)))
        .and_then(|(b, f)| f.call0(&b).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

impl NoteMeta {
    pub fn visible(&self) -> bool {
        !self.is_template && self.trashed == 0.0
    }

    /// Normales Notizbuch (keine Vorlage, kein Lernkarten-Set, nicht im Papierkorb).
    pub fn is_notebook(&self) -> bool {
        self.visible() && !self.is_deck
    }

    pub fn locked(&self) -> bool {
        !self.lock.is_empty()
    }
}

impl App {
    fn meta(&self, id: &str) -> Option<&NoteMeta> {
        self.library.iter().find(|m| m.id == id)
    }

    /// `true`, wenn das Notizbuch gesperrt und noch nicht entsperrt ist – dann wird nach der PIN gefragt.
    pub fn needs_unlock(&mut self, id: &str, then: Then) -> bool {
        let locked = self.meta(id).map(|m| m.locked()).unwrap_or(false);
        if !locked || self.unlocked.iter().any(|u| u == id) {
            return false;
        }
        let visual = self.cover_visual(id, "accent", "i-lock");
        let bio = bio_available();
        self.open_dialog(&visual, "Notizbuch gesperrt", if bio { "Entsperre mit deinem Fingerabdruck oder gib die PIN ein." } else { "Gib die PIN ein, um das Notizbuch zu öffnen." }, "Entsperren", false, Some(""), Pending::Unlock(id.to_string(), then));
        pin_input(true);
        show("dlg-bio", bio);
        if bio {
            self.start_bio();
        }
        true
    }

    pub fn start_bio(&mut self) {
        let title = match &self.pending {
            Some(Pending::Unlock(id, _)) => self.meta(id).map(|m| m.title.clone()).unwrap_or_default(),
            _ => return,
        };
        if let Some(b) = super::storage::android_bridge() {
            if let Some(f) = js_sys::Reflect::get(&b, &"authenticate".into()).ok().and_then(|f| f.dyn_into::<js_sys::Function>().ok()) {
                let _ = f.call1(&b, &JsValue::from_str(&title));
            }
        }
    }

    /// Ergebnis des Fingerabdrucks (aus der Android-App).
    pub fn bio_result(&mut self, ok: bool) {
        if !ok {
            return;
        }
        if let Some(Pending::Unlock(id, then)) = self.pending.take() {
            self.close_modals();
            self.unlock_and(id, then);
        }
    }

    fn unlock_and(&mut self, id: String, then: Then) {
        if !self.unlocked.contains(&id) {
            self.unlocked.push(id.clone());
        }
        match then {
            Then::Open => self.open_notebook(id),
            Then::Study => self.study_deck(id),
            Then::Card(act) => {
                self.ctx_target = Some(id);
                self.card_action(&act, 0);
            }
        }
    }

    pub fn open_guarded(&mut self, id: String) {
        if !self.needs_unlock(&id, Then::Open) {
            self.open_notebook(id);
        }
    }

    /// PIN-Dialoge bestätigt (aus `dialog_ok`). Gibt `false` zurück, wenn `pending` nicht hierher gehört.
    pub fn lock_dialog(&mut self, pending: &Pending, value: &str) -> bool {
        let pin: String = value.trim().to_string();
        match pending {
            Pending::Unlock(id, then) => {
                let stored = self.meta(id).map(|m| m.lock.clone()).unwrap_or_default();
                if pin_matches(&pin, &stored) {
                    self.unlock_and(id.clone(), then.clone());
                } else {
                    self.toast("Falsche PIN");
                    let (id, then) = (id.clone(), then.clone());
                    set_timeout(350, move || {
                        with_app(|a| a.needs_unlock(&id, then));
                    });
                }
            }
            Pending::SetPin(id) => {
                if pin.chars().count() < 4 || !pin.chars().all(|c| c.is_ascii_digit()) {
                    self.toast("Die PIN braucht mindestens 4 Ziffern");
                    let id = id.clone();
                    set_timeout(350, move || {
                        with_app(|a| a.ask_new_pin(id));
                    });
                    return true;
                }
                let visual = self.cover_visual(id, "accent", "i-lock");
                self.open_dialog(&visual, "PIN wiederholen", "Gib die PIN zur Bestätigung noch einmal ein.", "Sperren", false, Some(""), Pending::ConfirmPin(id.clone(), pin));
                pin_input(true);
            }
            Pending::ConfirmPin(id, first) => {
                if &pin != first {
                    self.toast("Die PINs stimmen nicht überein – bitte noch einmal");
                    let id = id.clone();
                    set_timeout(350, move || {
                        with_app(|a| a.ask_new_pin(id));
                    });
                    return true;
                }
                let salt = format!("{:08x}{:08x}", (js_sys::Math::random() * 4e9) as u32, (wall_clock() as u64 & 0xffff_ffff) as u32);
                let hash = pin_hash(&pin, &salt);
                let id = id.clone();
                self.unlocked.retain(|u| *u != id);
                self.modify_stored(id, move |nb| nb.lock = hash);
                self.toast("Notizbuch gesperrt – zum Öffnen wird jetzt die PIN abgefragt");
            }
            Pending::RemovePin(id) => {
                let stored = self.meta(id).map(|m| m.lock.clone()).unwrap_or_default();
                if pin_matches(&pin, &stored) {
                    self.modify_stored(id.clone(), |nb| nb.lock.clear());
                    self.toast("Sperre entfernt");
                } else {
                    self.toast("Falsche PIN – die Sperre bleibt");
                }
            }
            _ => return false,
        }
        true
    }

    pub fn ask_new_pin(&mut self, id: String) {
        let visual = self.cover_visual(&id, "accent", "i-lock");
        let bio = if bio_available() { " In der App kannst du danach auch mit dem Fingerabdruck entsperren." } else { "" };
        self.open_dialog(&visual, "Mit PIN sperren", &format!("Wähle eine PIN (mindestens 4 Ziffern).{}", bio), "Weiter", false, Some(""), Pending::SetPin(id));
        pin_input(true);
    }

    pub fn card_lock(&mut self, id: String) {
        let locked = self.meta(&id).map(|m| m.locked()).unwrap_or(false);
        if locked {
            let visual = self.cover_visual(&id, "accent", "i-unlock");
            self.open_dialog(&visual, "Sperre entfernen", "Gib die PIN ein, um die Sperre aufzuheben.", "Entfernen", false, Some(""), Pending::RemovePin(id));
            pin_input(true);
        } else {
            self.ask_new_pin(id);
        }
    }

    // ---------------------------------------------------------------
    // Papierkorb
    // ---------------------------------------------------------------

    pub fn trash_notebook(&mut self, id: String) {
        let now = wall_clock();
        self.modify_stored(id, move |nb| {
            nb.trashed = now;
            nb.favorite = false;
        });
        self.toast("In den Papierkorb verschoben – dort 30 Tage wiederherstellbar");
    }

    pub fn trash_count(&self) -> usize {
        self.library.iter().filter(|m| m.trashed > 0.0).count()
    }

    /// Papierkorb-Ansicht (statt der normalen Kacheln).
    pub fn render_trash(&mut self) -> String {
        let now = wall_clock();
        let mut list: Vec<&NoteMeta> = self.library.iter().filter(|m| m.trashed > 0.0).collect();
        list.sort_by(|a, b| b.trashed.partial_cmp(&a.trashed).unwrap_or(std::cmp::Ordering::Equal));
        let mut html = String::new();
        for m in list {
            let left = (TRASH_DAYS - (now - m.trashed) / DAY_MS).ceil().max(0.0) as i64;
            html.push_str(&format!(
                "<div class=\"card trashed\" data-act=\"trash-menu\" data-arg=\"{id}\">\
                   <div class=\"cover{pat}\" style=\"{css};aspect-ratio:{w} / {h}\">{art}<div class=\"cover-band\"></div><div class=\"cover-label\"><b>{title}</b><small>{pages} {pw}</small></div>\
                   <button class=\"card-more\" data-act=\"trash-menu\" data-arg=\"{id}\" title=\"Optionen\"><svg><use href=\"#i-more\"/></svg></button></div>\
                   <div class=\"card-meta\"><b>{title}</b><small>noch {left} {dw}</small></div>\
                 </div>",
                id = esc(&m.id),
                css = super::library::cover_css(m.cover),
                pat = super::library::cover_class(m.cover),
                art = super::library::cover_art(m.cover, m.w, m.h),
                w = m.w,
                h = m.h,
                title = esc(&m.title),
                pages = m.pages,
                pw = if m.pages == 1 { "Seite" } else { "Seiten" },
                left = left,
                dw = if left == 1 { "Tag" } else { "Tage" }
            ));
        }
        if html.is_empty() {
            html = "<p class=\"muted lib-none\">Der Papierkorb ist leer.</p>".into();
        }
        html
    }

    pub fn trash_action(&mut self, act: &str, id: String) {
        self.close_popovers();
        match act {
            "trash-restore" => {
                self.modify_stored(id, |nb| nb.trashed = 0.0);
                self.toast("Wiederhergestellt");
            }
            "trash-delete" => {
                let visual = self.cover_visual(&id, "danger", "i-trash");
                let title = self.meta(&id).map(|m| m.title.clone()).unwrap_or_default();
                self.open_dialog(&visual, "Endgültig löschen?", &format!("<b>„{}“</b> wird endgültig gelöscht. Das kann nicht rückgängig gemacht werden.", esc(&title)), "Endgültig löschen", true, None, Pending::DeleteForever(id));
            }
            _ => {}
        }
    }

    pub fn empty_trash_dialog(&mut self) {
        let n = self.trash_count();
        if n == 0 {
            return;
        }
        let visual = "<div class=\"dlg-icon danger\"><svg><use href=\"#i-trash\"/></svg></div>";
        self.open_dialog(visual, "Papierkorb leeren?", &format!("{} endgültig gelöscht. Das kann nicht rückgängig gemacht werden.", if n == 1 { "1 Eintrag wird".to_string() } else { format!("{} Einträge werden", n) }), "Papierkorb leeren", true, None, Pending::EmptyTrash);
    }

    pub fn empty_trash(&mut self) {
        let ids: Vec<String> = self.library.iter().filter(|m| m.trashed > 0.0).map(|m| m.id.clone()).collect();
        for id in ids {
            self.delete_stored(&id);
        }
        self.toast("Papierkorb geleert");
    }

    /// Beim Start: Einträge, die länger als 30 Tage im Papierkorb liegen, endgültig löschen.
    pub fn purge_trash(&mut self) {
        let now = wall_clock();
        let old: Vec<String> = self.library.iter().filter(|m| m.trashed > 0.0 && now - m.trashed > TRASH_DAYS * DAY_MS).map(|m| m.id.clone()).collect();
        for id in old {
            self.delete_stored(&id);
        }
    }
}
