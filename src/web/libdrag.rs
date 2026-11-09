//! Startbildschirm: Notizbücher per Ziehen in Ordner legen (oder heraus / in den Papierkorb).
//! Finger: kurz gedrückt halten, dann ziehen (normales Scrollen bleibt möglich).
//! Stift/Maus: direkt ziehen.

use super::*;
use std::cell::RefCell;
use web_sys::{Event, HtmlElement, PointerEvent};

/// Ablageziel unter dem Zeiger.
#[derive(Clone, PartialEq)]
enum Target {
    Folder(String),
    Root,
    Trash,
}

struct Drag {
    id: String,
    pointer: i32,
    touch: bool,
    start: (f64, f64),
    /// Abstand Zeiger ↔ linke obere Kartenecke
    grab: (f64, f64),
    active: bool,
    card: web_sys::Element,
    ghost: Option<HtmlElement>,
    over: Option<(Target, web_sys::Element)>,
    seq: u32,
}

thread_local! {
    static DRAG: RefCell<Option<Drag>> = const { RefCell::new(None) };
    static SEQ: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Klick nach dem Ablegen unterdrücken (sonst öffnet sich das Notizbuch)
    static SUPPRESS_CLICK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn elem(e: &web_sys::Event) -> Option<web_sys::Element> {
    e.target()?.dyn_into::<web_sys::Element>().ok()
}

/// Ablageziel an einem Fensterpunkt.
fn target_at(x: f64, y: f64) -> Option<(Target, web_sys::Element)> {
    let hit = dom().element_from_point(x as f32, y as f32)?;
    let t = hit.closest(".folder-card, #lib-crumb [data-act=folder-up], #lib-trash-btn").ok()??;
    let target = if t.class_list().contains("folder-card") {
        Target::Folder(t.get_attribute("data-arg")?)
    } else if t.id() == "lib-trash-btn" {
        Target::Trash
    } else {
        Target::Root
    };
    Some((target, t))
}

fn activate(d: &mut Drag) {
    d.active = true;
    vibrate(15);
    let Ok(node) = d.card.clone_node_with_deep(true) else { return };
    let Ok(ghost) = node.dyn_into::<HtmlElement>() else { return };
    let r = d.card.get_bounding_client_rect();
    let _ = ghost.class_list().add_1("drag-ghost");
    let _ = ghost.remove_attribute("data-act");
    style(&ghost, "width", &format!("{}px", r.width()));
    let _ = dom().body().map(|b| b.append_child(&ghost));
    let _ = d.card.class_list().add_1("dragging");
    set_class(&el("library"), "lib-dragging", true);
    d.ghost = Some(ghost);
}

fn move_ghost(d: &Drag, x: f64, y: f64) {
    if let Some(g) = &d.ghost {
        style(g, "transform", &format!("translate({}px,{}px) rotate(-3deg) scale(.92)", x - d.grab.0, y - d.grab.1));
    }
}

fn finish(d: Drag) {
    if let Some(g) = &d.ghost {
        g.remove();
    }
    let _ = d.card.class_list().remove_1("dragging");
    if let Some((_, t)) = &d.over {
        let _ = t.class_list().remove_1("drop-over");
    }
    set_class(&el("library"), "lib-dragging", false);
}

pub fn bind_lib_drag() {
    let lib = el("library");
    on(&lib, "pointerdown", |e: PointerEvent| {
        if !e.is_primary() || e.button() > 0 {
            return;
        }
        let Some(t) = elem(&e) else { return };
        // nicht über dem Optionen-Knopf der Karte
        if t.closest(".card-more").ok().flatten().is_some() {
            return;
        }
        let Some(card) = t.closest("#lib-grid .card[data-act=card]").ok().flatten() else { return };
        let Some(id) = card.get_attribute("data-arg") else { return };
        let in_trash = with_app(|a| a.show_trash).unwrap_or(true);
        if in_trash {
            return;
        }
        let r = card.get_bounding_client_rect();
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        let touch = e.pointer_type() == "touch";
        let seq = SEQ.with(|s| {
            s.set(s.get().wrapping_add(1));
            s.get()
        });
        DRAG.with(|d| {
            *d.borrow_mut() = Some(Drag { id, pointer: e.pointer_id(), touch, start: (x, y), grab: (x - r.left(), y - r.top()), active: false, card, ghost: None, over: None, seq });
        });
        if touch {
            // Finger: nach kurzem Halten anheben
            set_timeout(320, move || {
                DRAG.with(|d| {
                    if let Some(dr) = d.borrow_mut().as_mut() {
                        if dr.seq == seq && !dr.active {
                            activate(dr);
                            let (sx, sy) = dr.start;
                            move_ghost(dr, sx, sy);
                        }
                    }
                });
            });
        }
    });
    on(&lib, "pointermove", |e: PointerEvent| {
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        DRAG.with(|d| {
            let mut b = d.borrow_mut();
            let Some(dr) = b.as_mut() else { return };
            if dr.pointer != e.pointer_id() {
                return;
            }
            let moved = (x - dr.start.0).hypot(y - dr.start.1);
            if !dr.active {
                if dr.touch {
                    // vor dem Anheben bewegt → normales Scrollen
                    if moved > 10.0 {
                        *b = None;
                    }
                    return;
                }
                if moved < 8.0 {
                    return;
                }
                activate(dr);
            }
            e.prevent_default();
            move_ghost(dr, x, y);
            let over = target_at(x, y);
            let same = match (&over, &dr.over) {
                (Some((a, _)), Some((b, _))) => a == b,
                (None, None) => true,
                _ => false,
            };
            if !same {
                if let Some((_, t)) = &dr.over {
                    let _ = t.class_list().remove_1("drop-over");
                }
                if let Some((_, t)) = &over {
                    let _ = t.class_list().add_1("drop-over");
                }
                dr.over = over;
            }
        });
    });
    let end = |e: PointerEvent, cancel: bool| {
        let taken = DRAG.with(|d| {
            let mut b = d.borrow_mut();
            match b.as_ref() {
                Some(dr) if dr.pointer == e.pointer_id() => b.take(),
                _ => None,
            }
        });
        let Some(dr) = taken else { return };
        if !dr.active {
            return;
        }
        SUPPRESS_CLICK.with(|s| s.set(true));
        set_timeout(400, || SUPPRESS_CLICK.with(|s| s.set(false)));
        let id = dr.id.clone();
        let target = if cancel { None } else { dr.over.as_ref().map(|o| o.0.clone()) };
        finish(dr);
        if let Some(t) = target {
            with_app(|a| a.drop_notebook(id, t_folder(&t), matches!(t, Target::Trash)));
        }
    };
    on(&lib, "pointerup", move |e: PointerEvent| end(e, false));
    on(&lib, "pointercancel", move |e: PointerEvent| end(e, true));
    // während des Ziehens nicht scrollen
    on(&lib, "touchmove", |e: Event| {
        let active = DRAG.with(|d| d.borrow().as_ref().is_some_and(|dr| dr.active));
        if active {
            e.prevent_default();
        }
    });
    // Klick nach dem Ablegen abfangen (Erfassungsphase, vor dem allgemeinen Klick-Handler)
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        if SUPPRESS_CLICK.with(|s| s.replace(false)) {
            e.stop_propagation();
            e.prevent_default();
        }
    });
    let opts = AddEventListenerOptions::new();
    opts.set_capture(true);
    let _ = lib.add_event_listener_with_callback_and_add_event_listener_options("click", c.as_ref().unchecked_ref(), &opts);
    c.forget();
}

fn t_folder(t: &Target) -> String {
    match t {
        Target::Folder(f) => f.clone(),
        _ => String::new(),
    }
}

impl App {
    /// Abgelegtes Notizbuch verschieben (`folder` leer = Übersicht) oder in den Papierkorb legen.
    fn drop_notebook(&mut self, id: String, folder: String, trash: bool) {
        if trash {
            self.ctx_target = Some(id);
            self.card_action("card-delete", 0);
            return;
        }
        let Some(m) = self.library.iter().find(|m| m.id == id) else { return };
        if m.folder == folder {
            return;
        }
        let title = m.title.clone();
        // sofort in der Übersicht umsortieren, gespeichert wird im Hintergrund
        if let Some(m) = self.library.iter_mut().find(|m| m.id == id) {
            m.folder = folder.clone();
        }
        self.render_library();
        let label = if folder.is_empty() { "die Übersicht".to_string() } else { format!("„{}“", folder) };
        let f = folder.clone();
        self.modify_stored(id, move |nb| nb.folder = f);
        self.toast(&format!("„{}“ → {}", title, label));
    }
}
