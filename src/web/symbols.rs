//! Symbol-Palette: Mathe-, Technik- und Sonderzeichen zu einem Text zusammenstellen und einfügen.
//! Nur Zeichen, die auch der PDF-Export darstellen kann (Helvetica, Symbol, ZapfDingbats).

use super::*;
use crate::engine::{Content, TextBox};
use web_sys::HtmlInputElement;

const GROUPS: &[(&str, &[&str])] = &[
    ("Mathe", &["+", "−", "±", "×", "÷", "⋅", "=", "≠", "≈", "≡", "<", ">", "≤", "≥", "√", "∑", "∏", "∫", "∂", "∞", "π", "%", "‰", "°", "¹", "²", "³", "½", "¼", "¾", "∠", "⊥", "∆", "∅", "∈", "∉", "⊂", "∪", "∩", "∀", "∃"]),
    ("Griechisch", &["α", "β", "γ", "δ", "ε", "η", "θ", "λ", "μ", "ρ", "σ", "τ", "φ", "ω", "Δ", "Σ", "Φ", "Ω"]),
    ("Pfeile & Logik", &["→", "←", "↑", "↓", "↔", "⇒", "⇐", "⇔", "➔", "∧", "∨", "¬"]),
    ("Technik & Einheiten", &["€", "$", "£", "µ", "Ω", "°C", "⌀", "§", "©", "®", "™", "•", "…", "–"]),
    ("Markierungen", &["✓", "✔", "✗", "✘", "★", "☆", "●", "■", "▲", "◆", "❤", "✎", "☎", "✉"]),
];

fn sym_input() -> HtmlInputElement {
    el("sym-text").unchecked_into()
}

impl App {
    pub fn open_symbol_dialog(&mut self) {
        self.close_popovers();
        let box_ = el("sym-groups");
        if box_.child_element_count() == 0 {
            let mut h = String::new();
            for (name, syms) in GROUPS {
                h.push_str(&format!("<h4>{name}</h4><div class=\"sym-grid\">"));
                for s in *syms {
                    h.push_str(&format!("<button data-act=\"sym-add\" data-arg=\"{s}\">{s}</button>"));
                }
                h.push_str("</div>");
            }
            box_.set_inner_html(&h);
        }
        sym_input().set_value("");
        self.open_modal("modal-symbols");
    }

    pub fn symbol_action(&mut self, act: &str, arg: &str) {
        let input = sym_input();
        match act {
            "sym-add" => input.set_value(&format!("{}{}", input.value(), arg)),
            "sym-back" => {
                let mut v: Vec<char> = input.value().chars().collect();
                v.pop();
                input.set_value(&v.into_iter().collect::<String>());
            }
            "sym-space" => input.set_value(&format!("{} ", input.value())),
            "sym-insert" => {
                let text = input.value().trim_end().to_string();
                if text.is_empty() {
                    self.toast("Erst Zeichen antippen – sie erscheinen oben im Feld");
                    return;
                }
                self.close_modals();
                let size = self.prefs.text_size.max(10.0);
                let w = (text.chars().count() as f32 * size * 0.62 + 14.0).clamp(30.0, 520.0);
                let tb = TextBox { x: 0.0, y: 0.0, w, size, color: self.ink_color(), bold: false, text };
                self.insert_contents(vec![Content::Text(tb)], w, size * 1.5);
            }
            _ => {}
        }
    }
}
