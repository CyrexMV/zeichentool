//! Oberfläche: Werkzeugleisten, Regler, Pinsel-Bibliothek, Farbwähler, Ebenen, Menüs.

use super::*;
use crate::engine::brush::render_dab;
use crate::engine::history::StoredLayer;
use crate::engine::{Action, BlendMode, InputPoint, StrokeEngine, TemplateKind};
use web_sys::{HtmlSelectElement, PointerEvent};

const PANELS: [&str; 11] = [
    "panel-brush", "panel-color", "panel-layers", "panel-shape", "panel-fill", "panel-sym", "panel-menu", "panel-settings", "panel-help", "panel-paper", "modal-new",
];

const PALETTE: [&str; 32] = [
    "#000000", "#1d1d24", "#4a4a55", "#8b8b96", "#c9c9d1", "#ffffff", "#f6e6d0", "#e8c39e",
    "#c68b59", "#8d5524", "#5a3a22", "#ff3b30", "#ff6b6b", "#ff9500", "#ffcc00", "#ffe680",
    "#34c759", "#8be28b", "#0f7b4f", "#00c7be", "#30b0c7", "#32ade6", "#007aff", "#1e3a8a",
    "#5856d6", "#7c5cff", "#af52de", "#ff2d55", "#ff8ad8", "#a2845e", "#2c2c54", "#ffd6e0",
];

fn closest_attr(e: &Event, sel: &str, attr: &str) -> Option<String> {
    let t: web_sys::Element = e.target()?.dyn_into().ok()?;
    t.closest(sel).ok()??.get_attribute(attr)
}

fn bind_range(id: &'static str, f: impl Fn(&mut App, f32) + 'static) {
    let inp = input_el(id);
    on(&inp, "input", move |e: Event| {
        let v: HtmlInputElement = e.target().unwrap().unchecked_into();
        let n = v.value().parse::<f32>().unwrap_or(0.0);
        with_app(|a| f(a, n));
    });
}

fn bind_check(id: &'static str, f: impl Fn(&mut App, bool) + 'static) {
    let inp = input_el(id);
    on(&inp, "change", move |e: Event| {
        let v: HtmlInputElement = e.target().unwrap().unchecked_into();
        let c = v.checked();
        with_app(|a| f(a, c));
    });
}

fn set_out(id: &str, text: &str) {
    if let Some(e) = dom().get_element_by_id(id) {
        e.set_text_content(Some(text));
    }
}

fn set_range(id: &str, v: f32, label: &str) {
    input_el(id).set_value(&format!("{}", v.round() as i32));
    set_out(&format!("{}-v", id), label);
}

/// Vertikaler Regler (Größe / Deckkraft) mit Zeiger-Steuerung.
fn bind_vslider(id: &'static str, f: impl Fn(&mut App, f32) + 'static) {
    let e = el(id);
    let f = std::rc::Rc::new(f);
    let handler = {
        let e = e.clone();
        move |ev: &PointerEvent| {
            let r = e.get_bounding_client_rect();
            let (_, y) = super::input::client_xy(ev);
            (1.0 - (y - r.top()) / r.height()).clamp(0.0, 1.0) as f32
        }
    };
    let h1 = handler.clone();
    let f1 = f.clone();
    let e1 = e.clone();
    on(&e, "pointerdown", move |ev: PointerEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        let _ = e1.set_pointer_capture(ev.pointer_id());
        let t = h1(&ev);
        with_app(|a| {
            f1(a, t);
            a.show_bubble(id);
        });
    });
    let h2 = handler;
    let f2 = f;
    on(&e, "pointermove", move |ev: PointerEvent| {
        if ev.buttons() == 0 {
            return;
        }
        ev.prevent_default();
        let t = h2(&ev);
        with_app(|a| {
            f2(a, t);
            a.show_bubble(id);
        });
    });
    let hide = |_: PointerEvent| {
        set_class(&el("bubble"), "show", false);
        with_app(|a| a.save_settings());
    };
    on(&e, "pointerup", hide);
    on(&e, "pointercancel", hide);
}

pub fn max_size(p: &BrushPreset) -> f32 {
    if p.pixel {
        64.0
    } else {
        600.0
    }
}

pub fn bind_all() {
    // ----- Werkzeuge -----
    let tools = dom().query_selector_all(".tool").unwrap();
    for i in 0..tools.length() {
        let b: HtmlElement = tools.get(i).unwrap().unchecked_into();
        let code = b.get_attribute("data-tool").unwrap_or_default();
        on(&b, "click", move |_: Event| {
            with_app(|a| {
                if let Some(t) = Tool::from_code(&code) {
                    if a.tool == t {
                        match t {
                            Tool::Brush | Tool::Eraser | Tool::Smudge => a.toggle_panel("panel-brush"),
                            Tool::Fill => a.toggle_panel("panel-fill"),
                            Tool::Shape => a.toggle_panel("panel-shape"),
                            _ => {}
                        }
                    } else {
                        a.set_tool(t);
                        a.close_panels();
                    }
                }
            });
        });
    }
    on_click("btn-menu", |a| a.toggle_panel("panel-menu"));
    on_click("btn-undo", |a| a.undo());
    on_click("btn-redo", |a| a.redo());
    on_click("btn-zoom", |a| a.fit_view());
    on_click("btn-symmetry", |a| a.toggle_panel("panel-sym"));
    on_click("btn-layers", |a| a.toggle_panel("panel-layers"));
    on_click("btn-color", |a| {
        a.prev_color = a.color;
        a.toggle_panel("panel-color");
    });

    // ----- Seitenregler -----
    bind_vslider("vs-size", |a, t| {
        let max = max_size(&a.presets[a.slot().preset]);
        let s = 1.0 + (max - 1.0) * t.powf(2.2);
        a.slot_mut().size = if a.presets[a.slot().preset].pixel { s.round() } else { (s * 10.0).round() / 10.0 };
        a.refresh_side();
    });
    bind_vslider("vs-opacity", |a, t| {
        a.slot_mut().opacity = t.max(0.01);
        a.refresh_side();
    });

    bind_brush_panel();
    bind_color_panel();
    bind_layers_panel();

    // ----- Formen, Füllen, Symmetrie -----
    on(&el("shape-seg"), "click", |e: Event| {
        if let Some(s) = closest_attr(&e, "button", "data-shape") {
            with_app(|a| {
                a.settings.shape = ShapeKind::from_code(&s);
                a.refresh_settings_ui();
                a.save_settings();
            });
        }
    });
    bind_check("shape-fill", |a, v| {
        a.settings.shape_fill = v;
        a.save_settings();
    });
    bind_range("fill-tol", |a, v| {
        a.settings.fill_tol = v / 100.0;
        set_out("fill-tol-v", &format!("{}%", v as i32));
        a.save_settings();
    });
    bind_check("fill-all", |a, v| {
        a.settings.fill_all = v;
        a.save_settings();
    });
    on(&el("sym-seg"), "click", |e: Event| {
        if let Some(s) = closest_attr(&e, "button", "data-sym") {
            with_app(|a| {
                a.settings.symmetry = if s == "radial" { Symmetry::Radial(a.settings.radial_n) } else { Symmetry::from_code(&s) };
                a.after_symmetry_change();
            });
        }
    });
    bind_range("sym-n", |a, v| {
        a.settings.radial_n = v as u8;
        set_out("sym-n-v", &format!("{}", v as i32));
        if let Symmetry::Radial(_) = a.settings.symmetry {
            a.settings.symmetry = Symmetry::Radial(v as u8);
        }
        a.after_symmetry_change();
    });

    // ----- Menü -----
    on_click("m-new", |a| a.open_new_dialog());
    on_click("m-open", |a| {
        a.close_panels();
        input_el("file-open").click();
    });
    on_click("m-save", |a| {
        a.close_panels();
        a.download_project();
    });
    on_click("m-export", |a| {
        a.close_panels();
        a.export_png(true);
    });
    on_click("m-export-t", |a| {
        a.close_panels();
        a.export_png(false);
    });
    on_click("m-fit", |a| {
        a.close_panels();
        a.fit_view();
    });
    on_click("m-flip", |a| {
        a.close_panels();
        a.flip_view();
    });
    on_click("m-fullscreen", |a| {
        a.close_panels();
        let d = dom();
        if d.fullscreen_element().is_some() {
            d.exit_fullscreen();
        } else if let Some(root) = d.document_element() {
            let _ = root.request_fullscreen();
        }
    });
    on_click("m-settings", |a| a.toggle_panel("panel-settings"));
    on_click("m-help", |a| a.toggle_panel("panel-help"));
    on_click("m-paper", |a| a.toggle_panel("panel-paper"));
    on_click("ly-paper", |a| a.toggle_panel("panel-paper"));
    bind_paper_controls("pp", false);
    bind_paper_controls("new", true);

    // ----- Einstellungen -----
    bind_check("set-pen-only", |a, v| {
        a.settings.pen_only = v;
        a.save_settings();
    });
    bind_range("set-gamma", |a, v| {
        a.settings.gamma = v / 100.0;
        a.refresh_settings_ui();
        a.save_settings();
    });
    let sel: HtmlSelectElement = el("set-pen-button").unchecked_into();
    on(&sel, "change", |e: Event| {
        let s: HtmlSelectElement = e.target().unwrap().unchecked_into();
        let v = s.value();
        with_app(|a| {
            a.settings.pen_button = PenButton::from_code(&v);
            a.save_settings();
        });
    });
    bind_check("set-quickshape", |a, v| {
        a.settings.quickshape = v;
        a.save_settings();
    });
    bind_range("set-palm", |a, v| {
        a.settings.palm_lock = v / 10.0;
        a.refresh_settings_ui();
        a.save_settings();
    });
    bind_check("set-ignore-large", |a, v| {
        a.settings.ignore_large = v;
        a.save_settings();
    });
    bind_check("set-one-finger", |a, v| {
        a.settings.one_finger_pan = v;
        a.save_settings();
    });
    bind_check("set-hover", |a, v| {
        a.settings.hover_cursor = v;
        a.save_settings();
    });
    bind_check("set-side-right", |a, v| {
        a.settings.side_right = v;
        a.refresh_settings_ui();
        a.save_settings();
    });
    let pen_test = el("pen-test");
    let show_pen = |e: PointerEvent| {
        e.prevent_default();
        let p = e.pressure();
        let ty = e.pointer_type();
        let btn = e.buttons();
        let kind = match ty.as_str() {
            "pen" => "S-Pen",
            "touch" => "Finger",
            _ => "Maus",
        };
        let state = if btn & 1 != 0 { "berührt" } else { "schwebt" };
        let taste = if btn & 2 != 0 { " · Taste gedrückt" } else { "" };
        let gummi = if btn & 32 != 0 { " · Radiergummi" } else { "" };
        style(&el("pen-meter-fill"), "width", &format!("{}%", (p * 100.0).round()));
        set_out(
            "pen-info",
            &format!("{} {} · Druck {:.2} · Neigung {}°/{}°{}{}", kind, state, p, e.tilt_x(), e.tilt_y(), taste, gummi),
        );
    };
    on(&pen_test, "pointermove", show_pen);
    on(&pen_test, "pointerdown", show_pen);

    // ----- Neue Leinwand -----
    on(&el("new-presets"), "click", |e: Event| {
        let (Some(w), Some(h)) = (closest_attr(&e, "button", "data-w"), closest_attr(&e, "button", "data-h")) else { return };
        let (mut w, mut h) = (w.parse::<u32>().unwrap_or(0), h.parse::<u32>().unwrap_or(0));
        if w == 0 {
            let win = window();
            let dpr = win.device_pixel_ratio();
            w = (win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0) * dpr).round() as u32;
            h = (win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0) * dpr).round() as u32;
        }
        input_el("new-w").set_value(&w.to_string());
        input_el("new-h").set_value(&h.to_string());
    });
    on_click("new-cancel", |a| a.close_panels());
    on_click("new-create", |a| {
        let w = input_el("new-w").value().parse::<usize>().unwrap_or(0).clamp(16, 8192);
        let h = input_el("new-h").value().parse::<usize>().unwrap_or(0).clamp(16, 8192);
        if w * h > 8192 * 8192 / 2 {
            a.toast("Leinwand ist zu groß für den Speicher");
            return;
        }
        a.close_panels();
        let mut d = Doc::new(w, h);
        d.paper = a.new_paper;
        d.template = a.new_tpl;
        a.settings.last_template = a.new_tpl;
        a.settings.last_paper = a.new_paper;
        a.save_settings();
        a.replace_document(d);
        a.mark_unsaved();
        a.toast(&format!("Neue Leinwand {} × {}", w, h));
    });

    super::storage::bind_files();
}

// ---------------------------------------------------------------------------
// Pinsel-Bibliothek
// ---------------------------------------------------------------------------

fn bind_brush_panel() {
    with_app(|a| a.build_brush_list());
    on(&el("brush-list"), "click", |e: Event| {
        if let Some(i) = closest_attr(&e, ".brush-item", "data-idx").and_then(|s| s.parse::<usize>().ok()) {
            with_app(|a| a.select_preset(i));
        }
    });
    fn edit(id: &'static str, f: fn(&mut BrushPreset, f32)) {
        bind_range(id, move |a, v| {
            let i = a.slot().preset;
            f(&mut a.presets[i], v / 100.0);
            a.render_brush_preview(i);
            a.refresh_brush_controls();
            a.save_settings();
        });
    }
    edit("br-hardness", |p, v| p.hardness = v);
    edit("br-spacing", |p, v| p.spacing = v.max(0.02));
    edit("br-flow", |p, v| p.flow = v.max(0.01));
    edit("br-psize", |p, v| p.pressure_size = v);
    edit("br-popacity", |p, v| p.pressure_opacity = v);
    edit("br-grain", |p, v| p.grain = v);
    edit("br-tilt", |p, v| p.tilt_shading = v);
    bind_range("br-smoothing", |a, v| {
        a.settings.smoothing = v / 100.0;
        a.refresh_brush_controls();
        a.save_settings();
    });
    on_click("br-reset", |a| {
        let i = a.slot().preset;
        a.presets[i] = a.base_presets[i].clone();
        a.render_brush_preview(i);
        a.refresh_brush_controls();
        a.save_settings();
    });
}

impl App {
    fn build_brush_list(&mut self) {
        let mut html = String::new();
        let mut group = "";
        for (i, p) in self.presets.iter().enumerate() {
            if p.group != group {
                group = p.group;
                html.push_str(&format!("<div class=\"brush-group\">{}</div>", esc(group)));
            }
            html.push_str(&format!(
                "<button class=\"brush-item\" data-idx=\"{}\"><canvas width=\"200\" height=\"80\"></canvas><span class=\"bn\">{}</span></button>",
                i,
                esc(p.name)
            ));
        }
        el("brush-list").set_inner_html(&html);
        for i in 0..self.presets.len() {
            self.render_brush_preview(i);
        }
    }

    /// Zeichnet eine Beispiel-Linie mit simuliertem S-Pen-Druck.
    pub fn render_brush_preview(&mut self, i: usize) {
        let (w, h) = (200usize, 80usize);
        let Ok(Some(c)) = dom().query_selector(&format!(".brush-item[data-idx=\"{}\"] canvas", i)) else { return };
        let canvas: HtmlCanvasElement = c.unchecked_into();
        let Some(ctx) = canvas.get_context("2d").ok().flatten() else { return };
        let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
        let p = self.presets[i].clone();
        let size = if p.pixel { 5.0 } else { (p.default_size * 0.55).clamp(3.0, 30.0) };
        let opacity = p.default_opacity.max(0.4);
        let buildup = p.buildup;
        let mut m = Mask::new(w, h);
        let mut eng = StrokeEngine::new(p, size, 1.0, 0.0, Symmetry::Off, 0.0, 0.0);
        let mut dabs = Vec::new();
        for k in 0..=80 {
            let t = k as f32 / 80.0;
            let pt = InputPoint {
                x: 16.0 + 168.0 * t,
                y: 40.0 + 17.0 * (t * std::f32::consts::TAU).sin(),
                pressure: (t * std::f32::consts::PI).sin().max(0.08),
                tilt_x: 0.0,
                tilt_y: 0.0,
            };
            if k == 0 {
                eng.begin(pt, &mut dabs);
            } else {
                eng.add(pt, &mut dabs);
            }
        }
        eng.end(&mut dabs);
        for d in &dabs {
            render_dab(&mut m, d, buildup, &self.noise);
        }
        let mut rgba = vec![0u8; w * h * 4];
        for (j, px) in rgba.chunks_exact_mut(4).enumerate() {
            let a = m.data[j] as f32 / 65535.0 * opacity;
            px.copy_from_slice(&[240, 240, 248, (a * 255.0) as u8]);
        }
        if let Ok(img) = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&rgba[..]), w as u32, h as u32) {
            let _ = ctx.put_image_data(&img, 0.0, 0.0);
        }
    }

    fn select_preset(&mut self, i: usize) {
        match self.tool {
            Tool::Eraser => self.eraser.preset = i,
            Tool::Smudge => self.smudge.preset = i,
            _ => {
                let cur = self.brush.preset;
                self.preset_mem[cur] = (self.brush.size, self.brush.opacity);
                let (s, o) = self.preset_mem[i];
                self.brush = Slot { preset: i, size: s, opacity: o };
                if self.tool != Tool::Brush && self.tool != Tool::Shape {
                    self.set_tool(Tool::Brush);
                }
            }
        }
        let max = max_size(&self.presets[i]);
        let s = self.slot_mut();
        s.size = s.size.min(max);
        self.refresh_brush_controls();
        self.refresh_side();
        self.save_settings();
    }

    pub fn refresh_brush_controls(&mut self) {
        let i = self.slot().preset;
        let title = match self.tool {
            Tool::Eraser => "Radierer – Spitze",
            Tool::Smudge => "Wischfinger – Spitze",
            _ => "Pinsel",
        };
        set_out("brush-title", title);
        let items = dom().query_selector_all(".brush-item").unwrap();
        for k in 0..items.length() {
            let e: web_sys::Element = items.get(k).unwrap().unchecked_into();
            let idx = e.get_attribute("data-idx").and_then(|s| s.parse::<usize>().ok());
            set_class(&e, "active", idx == Some(i));
        }
        let p = self.presets[i].clone();
        let pc = |v: f32| format!("{}%", (v * 100.0).round() as i32);
        set_range("br-smoothing", self.settings.smoothing * 100.0, &pc(self.settings.smoothing));
        set_range("br-hardness", p.hardness * 100.0, &pc(p.hardness));
        set_range("br-spacing", p.spacing * 100.0, &pc(p.spacing));
        set_range("br-flow", p.flow * 100.0, &pc(p.flow));
        set_range("br-psize", p.pressure_size * 100.0, &pc(p.pressure_size));
        set_range("br-popacity", p.pressure_opacity * 100.0, &pc(p.pressure_opacity));
        set_range("br-grain", p.grain * 100.0, &pc(p.grain));
        set_range("br-tilt", p.tilt_shading * 100.0, &pc(p.tilt_shading));
    }

    // -----------------------------------------------------------------------
    // Werkzeuge & Seitenleiste
    // -----------------------------------------------------------------------

    pub fn set_tool(&mut self, t: Tool) {
        if self.stroke.is_some() {
            return;
        }
        self.tool = t;
        let tools = dom().query_selector_all(".tool").unwrap();
        for i in 0..tools.length() {
            let b: web_sys::Element = tools.get(i).unwrap().unchecked_into();
            set_class(&b, "active", b.get_attribute("data-tool").as_deref() == Some(t.code()));
        }
        self.refresh_side();
        self.refresh_brush_controls();
    }

    pub fn refresh_side(&mut self) {
        let slot = *self.slot();
        let max = max_size(&self.presets[slot.preset]);
        let t = ((slot.size - 1.0).max(0.0) / (max - 1.0)).powf(1.0 / 2.2).clamp(0.0, 1.0);
        let set = |id: &str, t: f32| {
            let e = el(id);
            let fill: HtmlElement = e.query_selector(".vs-fill").unwrap().unwrap().unchecked_into();
            let thumb: HtmlElement = e.query_selector(".vs-thumb").unwrap().unwrap().unchecked_into();
            style(&fill, "height", &format!("{}%", t * 100.0));
            style(&thumb, "bottom", &format!("{}%", t * 100.0));
        };
        set("vs-size", t);
        set("vs-opacity", slot.opacity);
        set_out("vs-size-label", &format_size(slot.size));
        set_out("vs-opacity-label", &format!("{}%", (slot.opacity * 100.0).round() as i32));
        set_class(&el("side"), "right", self.settings.side_right);
    }

    fn show_bubble(&mut self, id: &str) {
        let b = el("bubble");
        let slot = *self.slot();
        let r = el(id).get_bounding_client_rect();
        if id == "vs-size" {
            let d = (slot.size as f64 * self.view.zoom()).clamp(2.0, 160.0);
            b.set_inner_html(&format!(
                "<span class=\"dot\" style=\"width:{d}px;height:{d}px;opacity:{o}\"></span><span>{s} px</span>",
                d = d,
                o = slot.opacity,
                s = format_size(slot.size)
            ));
        } else {
            b.set_inner_html(&format!("<span>Deckkraft {}%</span>", (slot.opacity * 100.0).round() as i32));
        }
        let left = if self.settings.side_right { r.left() - 220.0 } else { r.right() + 22.0 };
        style(&b, "left", &format!("{}px", left));
        style(&b, "top", &format!("{}px", r.top() + r.height() * 0.5 - 30.0));
        set_class(&b, "show", true);
    }

    // -----------------------------------------------------------------------
    // Panels
    // -----------------------------------------------------------------------

    pub fn toggle_panel(&mut self, id: &'static str) {
        let open = self.open_panel == Some(id);
        self.close_panels();
        if !open {
            set_class(&el(id), "open", true);
            self.open_panel = Some(id);
            match id {
                "panel-layers" => self.render_layers(),
                "panel-color" => self.refresh_color_ui(),
                "panel-brush" => self.refresh_brush_controls(),
                "panel-paper" => self.refresh_paper_ui(),
                _ => {}
            }
        }
    }

    pub fn close_panels(&mut self) {
        for p in PANELS {
            set_class(&el(p), "open", false);
        }
        self.open_panel = None;
    }

    fn open_new_dialog(&mut self) {
        self.close_panels();
        self.new_paper = self.doc.paper;
        self.new_tpl = self.doc.template;
        self.refresh_paper_ui();
        input_el("new-w").set_value(&self.doc.w.to_string());
        input_el("new-h").set_value(&self.doc.h.to_string());
        set_class(&el("modal-new"), "open", true);
        self.open_panel = Some("modal-new");
    }

    pub fn refresh_history_buttons(&mut self) {
        let u: web_sys::HtmlButtonElement = el("btn-undo").unchecked_into();
        let r: web_sys::HtmlButtonElement = el("btn-redo").unchecked_into();
        u.set_disabled(!self.history.can_undo());
        r.set_disabled(!self.history.can_redo());
    }

    pub fn refresh_settings_ui(&mut self) {
        let s = &self.settings;
        input_el("set-pen-only").set_checked(s.pen_only);
        input_el("set-quickshape").set_checked(s.quickshape);
        input_el("set-hover").set_checked(s.hover_cursor);
        input_el("set-ignore-large").set_checked(s.ignore_large);
        input_el("set-one-finger").set_checked(s.one_finger_pan);
        set_range("set-palm", s.palm_lock * 10.0, &format!("{:.1} s", s.palm_lock));
        input_el("set-side-right").set_checked(s.side_right);
        let desc = if s.gamma < 0.8 {
            "weich – wenig Druck reicht"
        } else if s.gamma > 1.3 {
            "hart – mehr Druck nötig"
        } else {
            "normal"
        };
        set_range("set-gamma", s.gamma * 100.0, &format!("{:.1}", s.gamma));
        set_out("set-gamma-desc", desc);
        let sel: HtmlSelectElement = el("set-pen-button").unchecked_into();
        sel.set_value(s.pen_button.code());
        input_el("shape-fill").set_checked(s.shape_fill);
        set_range("fill-tol", s.fill_tol * 100.0, &format!("{}%", (s.fill_tol * 100.0).round() as i32));
        input_el("fill-all").set_checked(s.fill_all);
        set_range("sym-n", s.radial_n as f32, &s.radial_n.to_string());
        let shape = s.shape.code();
        let sym = match s.symmetry {
            Symmetry::Radial(_) => "radial".to_string(),
            o => o.code(),
        };
        let mark = |sel: &str, attr: &str, val: &str| {
            let list = dom().query_selector_all(sel).unwrap();
            for i in 0..list.length() {
                let b: web_sys::Element = list.get(i).unwrap().unchecked_into();
                set_class(&b, "active", b.get_attribute(attr).as_deref() == Some(val));
            }
        };
        mark("#shape-seg button", "data-shape", shape);
        mark("#sym-seg button", "data-sym", &sym);
        set_class(&el("btn-symmetry"), "active", s.symmetry != Symmetry::Off);
        set_class(&el("side"), "right", s.side_right);
    }

    fn after_symmetry_change(&mut self) {
        self.update_guides();
        self.refresh_settings_ui();
        self.save_settings();
    }

    pub fn update_guides(&mut self) {
        let (w, h) = (self.doc.w as f32, self.doc.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let line = |x1: f32, y1: f32, x2: f32, y2: f32| format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>", x1, y1, x2, y2);
        let mut s = String::new();
        match self.settings.symmetry {
            Symmetry::Off => {}
            Symmetry::Vertical => s += &line(cx, 0.0, cx, h),
            Symmetry::Horizontal => s += &line(0.0, cy, w, cy),
            Symmetry::Quad => {
                s += &line(cx, 0.0, cx, h);
                s += &line(0.0, cy, w, cy);
            }
            Symmetry::Radial(n) => {
                let r = (w * w + h * h).sqrt();
                for k in 0..n {
                    let a = std::f32::consts::TAU * k as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                    s += &line(cx, cy, cx + a.cos() * r, cy + a.sin() * r);
                }
            }
        }
        if self.settings.symmetry != Symmetry::Off {
            s += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/>", cx, cy, (w.max(h) / 300.0).max(3.0));
        }
        el("guides").set_inner_html(&s);
    }

    pub fn refresh_all_ui(&mut self) {
        let t = self.tool;
        self.set_tool(t);
        self.refresh_color_ui();
        self.refresh_settings_ui();
        self.refresh_history_buttons();
        self.update_guides();
        self.render_layers();
        self.render_palette();
    }

    // -----------------------------------------------------------------------
    // Farbe
    // -----------------------------------------------------------------------

    pub fn set_color(&mut self, c: Color, from_hsv: bool) {
        self.color = c;
        if !from_hsv {
            let (h, s, v) = c.to_hsv();
            let h = if s < 0.01 || v < 0.01 { self.hsv.0 } else { h };
            self.hsv = (h, s, v);
        }
        self.refresh_color_ui();
    }

    pub fn push_recent(&mut self, c: Color) {
        if self.recent.first() == Some(&c) {
            return;
        }
        self.recent.retain(|x| x.to_hex() != c.to_hex());
        self.recent.insert(0, c);
        self.recent.truncate(18);
        self.render_recent();
        self.save_settings();
    }

    fn render_palette(&mut self) {
        let html: String = PALETTE.iter().map(|h| format!("<button class=\"sw\" data-hex=\"{0}\" style=\"background:{0}\"></button>", h)).collect();
        el("palette").set_inner_html(&html);
        self.render_recent();
    }

    fn render_recent(&mut self) {
        let html: String = self
            .recent
            .iter()
            .map(|c| format!("<button class=\"sw\" data-hex=\"{0}\" style=\"background:{0}\"></button>", c.to_hex()))
            .collect();
        el("recent").set_inner_html(if html.is_empty() { "<span class=\"empty\">Noch keine – wird beim Zeichnen gefüllt</span>" } else { &html });
    }

    pub fn refresh_color_ui(&mut self) {
        let hex = self.color.to_hex();
        style(&el("color-dot"), "background", &hex);
        style(&el("col-new"), "background", &hex);
        style(&el("col-old"), "background", &self.prev_color.to_hex());
        let hexin = input_el("hex");
        if dom().active_element().map(|e| e.id()) != Some("hex".into()) {
            hexin.set_value(&hex.to_uppercase());
        }
        let (h, s, v) = self.hsv;
        style(&el("sv"), "background", &Color::from_hsv(h, 1.0, 1.0).to_hex());
        let k = el("sv-knob");
        style(&k, "left", &format!("{}%", s * 100.0));
        style(&k, "top", &format!("{}%", (1.0 - v) * 100.0));
        style(&k, "background", &hex);
        let hk = el("hue-knob");
        style(&hk, "left", &format!("{}%", h / 360.0 * 100.0));
        style(&hk, "background", &Color::from_hsv(h, 1.0, 1.0).to_hex());
    }
}

fn format_size(s: f32) -> String {
    if s < 10.0 {
        format!("{:.1}", s)
    } else {
        format!("{:.0}", s)
    }
}

fn bind_color_panel() {
    fn area(id: &'static str, f: fn(&mut App, f32, f32)) {
        let e = el(id);
        let e2 = e.clone();
        let handle = move |ev: PointerEvent, down: bool| {
            if !down && ev.buttons() == 0 {
                return;
            }
            ev.prevent_default();
            if down {
                let _ = e2.set_pointer_capture(ev.pointer_id());
            }
            let r = e2.get_bounding_client_rect();
            let (x, y) = super::input::client_xy(&ev);
            let fx = ((x - r.left()) / r.width()).clamp(0.0, 1.0) as f32;
            let fy = ((y - r.top()) / r.height()).clamp(0.0, 1.0) as f32;
            with_app(|a| f(a, fx, fy));
        };
        let h1 = handle.clone();
        on(&e, "pointerdown", move |ev: PointerEvent| h1(ev, true));
        on(&e, "pointermove", move |ev: PointerEvent| handle(ev, false));
        on(&e, "pointerup", |_: PointerEvent| {
            with_app(|a| a.save_settings());
        });
    }
    area("sv", |a, x, y| {
        a.hsv.1 = x;
        a.hsv.2 = 1.0 - y;
        let (h, s, v) = a.hsv;
        a.set_color(Color::from_hsv(h, s, v), true);
    });
    area("hue", |a, x, _| {
        a.hsv.0 = (x * 360.0).min(359.9);
        let (h, s, v) = a.hsv;
        a.set_color(Color::from_hsv(h, s, v), true);
    });
    on(&el("hex"), "change", |e: Event| {
        let v: HtmlInputElement = e.target().unwrap().unchecked_into();
        let s = v.value();
        with_app(|a| match Color::from_hex(&s) {
            Some(c) => {
                a.set_color(c, false);
                a.push_recent(c);
            }
            None => a.toast("Ungültige Farbe – Beispiel: #3A7BFF"),
        });
    });
    let pick = |e: Event| {
        if let Some(h) = closest_attr(&e, ".sw", "data-hex") {
            with_app(|a| {
                if let Some(c) = Color::from_hex(&h) {
                    a.set_color(c, false);
                    a.save_settings();
                }
            });
        }
    };
    on(&el("palette"), "click", pick);
    on(&el("recent"), "click", pick);
    on_click("col-old", |a| {
        let c = a.prev_color;
        a.set_color(c, false);
    });
}

// ---------------------------------------------------------------------------
// Ebenen
// ---------------------------------------------------------------------------

fn bind_layers_panel() {
    let sel: HtmlSelectElement = el("ly-blend").unchecked_into();
    let opts: String = BlendMode::ALL.iter().map(|m| format!("<option value=\"{}\">{}</option>", *m as u8, m.name_de())).collect();
    sel.set_inner_html(&opts);

    on(&el("layers-list"), "click", |e: Event| {
        let act = closest_attr(&e, "[data-act]", "data-act");
        let idx = closest_attr(&e, ".layer-row", "data-idx").and_then(|s| s.parse::<usize>().ok());
        let on_name = closest_attr(&e, ".ln", "class").is_some();
        let Some(i) = idx else { return };
        with_app(|a| {
            if i >= a.doc.layers.len() {
                return;
            }
            if act.as_deref() == Some("vis") {
                let l = &mut a.doc.layers[i];
                l.props.visible = !l.props.visible;
                a.redraw_all();
                a.mark_unsaved();
                a.render_layers();
            } else if on_name && a.doc.active == i {
                a.rename_layer(i);
            } else {
                a.doc.active = i;
                a.render_layers();
            }
        });
    });

    on_click("ly-add", |a| {
        if a.doc.layers.len() >= 48 {
            a.toast("Maximale Anzahl an Ebenen erreicht");
            return;
        }
        let name = a.doc.next_layer_name();
        let idx = a.doc.add_layer(&name, a.doc.active + 1);
        let stored = StoredLayer::from_layer(&a.doc.layers[idx]);
        a.history.push(Action::AddLayer { index: idx, layer: stored });
        a.layers_changed();
    });
    on_click("ly-dup", |a| {
        let idx = a.doc.duplicate_layer(a.doc.active);
        let stored = StoredLayer::from_layer(&a.doc.layers[idx]);
        a.history.push(Action::AddLayer { index: idx, layer: stored });
        a.layers_changed();
    });
    on_click("ly-del", |a| {
        if a.doc.layers.len() <= 1 {
            a.toast("Die letzte Ebene kann nicht gelöscht werden");
            return;
        }
        let i = a.doc.active;
        let stored = StoredLayer::from_layer(&a.doc.layers[i]);
        a.doc.layers.remove(i);
        a.doc.active = i.saturating_sub(1).min(a.doc.layers.len() - 1);
        a.history.push(Action::RemoveLayer { index: i, layer: stored });
        a.layers_changed();
        a.toast("Ebene gelöscht – Rückgängig möglich");
    });
    on_click("ly-merge", |a| {
        let i = a.doc.active;
        if i == 0 {
            a.toast("Keine Ebene darunter");
            return;
        }
        let full = a.doc.bounds();
        let upper = StoredLayer::from_layer(&a.doc.layers[i]);
        let lower_id = a.doc.layers[i - 1].id;
        let before = a.doc.read_region(i - 1, full);
        a.doc.merge_down(i);
        let after = a.doc.read_region(i - 1, full);
        a.history.push(Action::Compound(vec![
            Action::patch(lower_id, full, &before, &after),
            Action::RemoveLayer { index: i, layer: upper },
        ]));
        a.layers_changed();
    });
    on_click("ly-clear", |a| {
        if a.doc.active_layer().props.locked {
            a.toast("Ebene ist gesperrt");
            return;
        }
        let i = a.doc.active;
        let full = a.doc.bounds();
        let before = a.doc.read_region(i, full);
        let l = a.doc.active_layer_mut();
        l.data.fill(0);
        l.touch();
        let after = a.doc.read_region(i, full);
        let id = a.doc.layers[i].id;
        a.history.push(Action::patch(id, full, &before, &after));
        a.layers_changed();
    });
    fn mv(a: &mut App, up: bool) {
        let i = a.doc.active;
        let j = if up { i + 1 } else { i.wrapping_sub(1) };
        if j >= a.doc.layers.len() {
            return;
        }
        let l = a.doc.layers.remove(i);
        a.doc.layers.insert(j, l);
        a.doc.active = j;
        a.history.push(Action::MoveLayer { from: i, to: j });
        a.layers_changed();
    }
    on_click("ly-up", |a| mv(a, true));
    on_click("ly-down", |a| mv(a, false));
    fn flip(a: &mut App, horizontal: bool) {
        if a.doc.active_layer().props.locked {
            a.toast("Ebene ist gesperrt");
            return;
        }
        let i = a.doc.active;
        let full = a.doc.bounds();
        let before = a.doc.read_region(i, full);
        a.doc.flip_layer(i, horizontal);
        let after = a.doc.read_region(i, full);
        let id = a.doc.layers[i].id;
        a.history.push(Action::patch(id, full, &before, &after));
        a.layers_changed();
    }
    on_click("ly-flip-h", |a| flip(a, true));
    on_click("ly-flip-v", |a| flip(a, false));
    on_click("ly-import", |a| {
        let _ = a;
        input_el("file-import").click();
    });

    // Eigenschaften der aktiven Ebene
    let op = input_el("ly-opacity");
    on(&op, "input", |e: Event| {
        let v: HtmlInputElement = e.target().unwrap().unchecked_into();
        let n = v.value().parse::<f32>().unwrap_or(100.0) / 100.0;
        with_app(|a| {
            if a.props_before.is_none() {
                a.props_before = Some(a.doc.active_layer().props.clone());
            }
            a.doc.active_layer_mut().props.opacity = n;
            set_out("ly-opacity-v", &format!("{}%", (n * 100.0).round() as i32));
            a.redraw_all();
        });
    });
    on(&op, "change", |_: Event| {
        with_app(|a| {
            if let Some(before) = a.props_before.take() {
                let l = a.doc.active_layer();
                a.history.push(Action::Props { layer: l.id, before, after: l.props.clone() });
                a.after_change();
            }
        });
    });
    on(&sel, "change", |e: Event| {
        let s: HtmlSelectElement = e.target().unwrap().unchecked_into();
        let m = BlendMode::from_u8(s.value().parse::<u8>().unwrap_or(0));
        with_app(|a| a.change_props(|p| p.blend = m));
    });
    bind_check("ly-alpha", |a, v| a.change_props(|p| p.alpha_lock = v));
    bind_check("ly-lock", |a, v| a.change_props(|p| p.locked = v));
    let paper_on = input_el("paper-on");
    on(&paper_on, "change", |e: Event| {
        let v: HtmlInputElement = e.target().unwrap().unchecked_into();
        let c = v.checked();
        let col = Color::from_hex(&input_el("paper-color").value()).unwrap_or(Color::WHITE);
        with_app(|a| {
            a.doc.paper = if c { Some(col) } else { None };
            a.redraw_all();
            a.mark_unsaved();
        });
    });
    on(&el("paper-color"), "input", |e: Event| {
        let v: HtmlInputElement = e.target().unwrap().unchecked_into();
        let col = Color::from_hex(&v.value()).unwrap_or(Color::WHITE);
        input_el("paper-on").set_checked(true);
        with_app(|a| {
            a.doc.paper = Some(col);
            a.redraw_all();
            a.mark_unsaved();
        });
    });
}

impl App {
    fn layers_changed(&mut self) {
        self.redraw_all();
        self.after_change();
        self.render_layers();
    }

    fn change_props(&mut self, f: impl FnOnce(&mut crate::engine::LayerProps)) {
        let l = self.doc.active_layer_mut();
        let before = l.props.clone();
        f(&mut l.props);
        let after = l.props.clone();
        let id = l.id;
        if before != after {
            self.history.push(Action::Props { layer: id, before, after });
            self.redraw_all();
            self.after_change();
            self.render_layers();
        }
    }

    fn rename_layer(&mut self, i: usize) {
        let cur = self.doc.layers[i].props.name.clone();
        if let Ok(Some(name)) = window().prompt_with_message_and_default("Ebene umbenennen", &cur) {
            let name = name.trim().chars().take(40).collect::<String>();
            if !name.is_empty() {
                self.change_props(|p| p.name = name);
            }
        }
    }

    pub fn render_layers(&mut self) {
        self.ui_layers_dirty = false;
        self.last_ui_refresh = now();
        let n = self.doc.layers.len();
        let mut html = String::new();
        for i in (0..n).rev() {
            let l = &self.doc.layers[i];
            let p = &l.props;
            let mut meta = format!("{} · {}%", p.blend.name_de(), (p.opacity * 100.0).round() as i32);
            if p.locked {
                meta.push_str(" · 🔒");
            }
            if p.alpha_lock {
                meta.push_str(" · α");
            }
            html.push_str(&format!(
                "<div class=\"layer-row{a}{h}\" data-idx=\"{i}\"><canvas width=\"120\" height=\"88\"></canvas><div class=\"ln\"><b>{name}</b><small>{meta}</small></div><button class=\"ib\" data-act=\"vis\" title=\"Sichtbarkeit\"><svg><use href=\"#{icon}\"/></svg></button></div>",
                a = if i == self.doc.active { " active" } else { "" },
                h = if p.visible { "" } else { " hidden" },
                i = i,
                name = esc(&p.name),
                meta = meta,
                icon = if p.visible { "i-eye" } else { "i-eye-off" },
            ));
        }
        el("layers-list").set_inner_html(&html);
        // Vorschaubilder
        let (dw, dh) = (self.doc.w as f32, self.doc.h as f32);
        let s = (120.0 / dw).min(88.0 / dh);
        let (tw, th) = (((dw * s).round() as usize).max(1), ((dh * s).round() as usize).max(1));
        let (ox, oy) = ((120 - tw.min(120)) / 2, (88 - th.min(88)) / 2);
        for i in 0..n {
            let Ok(Some(c)) = dom().query_selector(&format!(".layer-row[data-idx=\"{}\"] canvas", i)) else { continue };
            let canvas: HtmlCanvasElement = c.unchecked_into();
            let Some(ctx) = canvas.get_context("2d").ok().flatten() else { continue };
            let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
            let px = self.doc.thumbnail(i, tw, th);
            if let Ok(img) = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&px[..]), tw as u32, th as u32) {
                let _ = ctx.put_image_data(&img, ox as f64, oy as f64);
            }
        }
        // Eigenschaften der aktiven Ebene
        let p = self.doc.active_layer().props.clone();
        set_range("ly-opacity", p.opacity * 100.0, &format!("{}%", (p.opacity * 100.0).round() as i32));
        let sel: HtmlSelectElement = el("ly-blend").unchecked_into();
        sel.set_value(&(p.blend as u8).to_string());
        input_el("ly-alpha").set_checked(p.alpha_lock);
        input_el("ly-lock").set_checked(p.locked);
        input_el("paper-on").set_checked(self.doc.paper.is_some());
        input_el("paper-color").set_value(&self.doc.paper.unwrap_or(Color::WHITE).to_hex());
    }
}

// ---------------------------------------------------------------------------
// Papier & Vorlage
// ---------------------------------------------------------------------------

/// Bindet Vorlagen-, Abstands- und Papierfarb-Auswahl. `for_new` = Dialog „Neue Leinwand“.
fn bind_paper_controls(prefix: &'static str, for_new: bool) {
    on(&el(&format!("{}-tpl", prefix)), "click", move |e: Event| {
        if let Some(code) = closest_attr(&e, "button", "data-tpl") {
            with_app(|a| {
                let kind = TemplateKind::from_code(&code);
                if for_new {
                    a.new_tpl.kind = kind;
                    if kind != TemplateKind::Blank && a.new_paper.is_none() {
                        a.new_paper = Some(Color::WHITE);
                    }
                } else {
                    a.doc.template.kind = kind;
                    if kind != TemplateKind::Blank && a.doc.paper.is_none() {
                        a.doc.paper = Some(Color::WHITE);
                    }
                    a.paper_changed();
                }
                a.refresh_paper_ui();
            });
        }
    });
    on(&el(&format!("{}-sp", prefix)), "click", move |e: Event| {
        if let Some(sp) = closest_attr(&e, "button", "data-sp").and_then(|v| v.parse::<u8>().ok()) {
            with_app(|a| {
                if for_new {
                    a.new_tpl.spacing = sp.min(2);
                } else {
                    a.doc.template.spacing = sp.min(2);
                    a.paper_changed();
                }
                a.refresh_paper_ui();
            });
        }
    });
    on(&el(&format!("{}-col", prefix)), "click", move |e: Event| {
        if let Some(v) = closest_attr(&e, "button", "data-paper") {
            let col = Color::from_hex(&v);
            with_app(|a| {
                if for_new {
                    a.new_paper = col;
                } else {
                    a.doc.paper = col;
                    a.paper_changed();
                }
                a.refresh_paper_ui();
            });
        }
    });
}

impl App {
    fn paper_changed(&mut self) {
        self.settings.last_template = self.doc.template;
        self.settings.last_paper = self.doc.paper;
        self.redraw_all();
        self.mark_unsaved();
        self.save_settings();
        if self.open_panel == Some("panel-layers") {
            self.render_layers();
        }
    }

    pub fn refresh_paper_ui(&mut self) {
        let mark = |sel: &str, attr: &str, val: &str| {
            let list = dom().query_selector_all(sel).unwrap();
            for i in 0..list.length() {
                let b: web_sys::Element = list.get(i).unwrap().unchecked_into();
                set_class(&b, "active", b.get_attribute(attr).as_deref() == Some(val));
            }
        };
        let paper_code = |p: Option<Color>| p.map(|c| c.to_hex()).unwrap_or_else(|| "none".into());
        let (dt, np) = (self.doc.template, self.new_tpl);
        mark("#pp-tpl button", "data-tpl", dt.kind.code());
        mark("#pp-sp button", "data-sp", &dt.spacing.to_string());
        mark("#pp-col button", "data-paper", &paper_code(self.doc.paper));
        mark("#new-tpl button", "data-tpl", np.kind.code());
        mark("#new-sp button", "data-sp", &np.spacing.to_string());
        mark("#new-col button", "data-paper", &paper_code(self.new_paper));
    }
}
