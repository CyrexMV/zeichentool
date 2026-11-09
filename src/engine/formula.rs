//! Einfache Formeln mit Brüchen: „x = (a+b)/(c-d) + 1/2“ → Text- und Bruch-Abschnitte.

#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Text(String),
    /// Zähler, Nenner
    Frac(String, String),
}

/// Zeichen, die einen Bruchteil beenden (Rechenzeichen, Leerzeichen, Klammern).
fn is_break(c: char) -> bool {
    c.is_whitespace() || matches!(c, '+' | '-' | '−' | '=' | '*' | '×' | '·' | '⋅' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | ',' | ';' | '(' | ')' | '/')
}

/// Formel zerlegen. Ein „/“ macht aus dem Ausdruck davor und danach einen Bruch;
/// geklammerte Ausdrücke werden als Ganzes genommen (die Klammern entfallen).
pub fn parse(s: &str) -> Vec<Seg> {
    let ch: Vec<char> = s.chars().collect();
    let mut out: Vec<Seg> = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < ch.len() {
        if ch[i] != '/' {
            text.push(ch[i]);
            i += 1;
            continue;
        }
        // Zähler: vom Ende des bisherigen Texts zurück
        let tc: Vec<char> = text.chars().collect();
        let mut end = tc.len();
        while end > 0 && tc[end - 1] == ' ' {
            end -= 1;
        }
        let (num, start) = if end > 0 && tc[end - 1] == ')' {
            let mut depth = 0;
            let mut k = end;
            let mut found = None;
            while k > 0 {
                k -= 1;
                match tc[k] {
                    ')' => depth += 1,
                    '(' => {
                        depth -= 1;
                        if depth == 0 {
                            found = Some(k);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            match found {
                Some(k) => (tc[k + 1..end - 1].iter().collect::<String>(), k),
                None => (String::new(), end),
            }
        } else {
            let mut k = end;
            while k > 0 && !is_break(tc[k - 1]) {
                k -= 1;
            }
            (tc[k..end].iter().collect::<String>(), k)
        };
        // Nenner: ab hier vorwärts
        let mut j = i + 1;
        while j < ch.len() && ch[j] == ' ' {
            j += 1;
        }
        let (den, next) = if j < ch.len() && ch[j] == '(' {
            let mut depth = 0;
            let mut k = j;
            let mut found = None;
            while k < ch.len() {
                match ch[k] {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            found = Some(k);
                            break;
                        }
                    }
                    _ => {}
                }
                k += 1;
            }
            match found {
                Some(k) => (ch[j + 1..k].iter().collect::<String>(), k + 1),
                None => (String::new(), j),
            }
        } else {
            let mut k = j;
            while k < ch.len() && !is_break(ch[k]) {
                k += 1;
            }
            (ch[j..k].iter().collect::<String>(), k)
        };
        if num.trim().is_empty() || den.trim().is_empty() {
            // kein vollständiger Bruch → „/“ als normales Zeichen
            text.push('/');
            i += 1;
            continue;
        }
        let before: String = tc[..start].iter().collect();
        if !before.is_empty() {
            out.push(Seg::Text(before));
        }
        text.clear();
        out.push(Seg::Frac(num.trim().to_string(), den.trim().to_string()));
        i = next;
    }
    if !text.is_empty() {
        out.push(Seg::Text(text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions() {
        assert_eq!(parse("1/2"), vec![Seg::Frac("1".into(), "2".into())]);
        assert_eq!(
            parse("x = (a+b)/(c-d) + 1/2"),
            vec![Seg::Text("x = ".into()), Seg::Frac("a+b".into(), "c-d".into()), Seg::Text(" + ".into()), Seg::Frac("1".into(), "2".into())]
        );
        assert_eq!(parse("U/I = R"), vec![Seg::Frac("U".into(), "I".into()), Seg::Text(" = R".into())]);
        assert_eq!(parse("a /"), vec![Seg::Text("a /".into())]);
        assert_eq!(parse("ohne Bruch"), vec![Seg::Text("ohne Bruch".into())]);
        assert_eq!(parse("3/(4x)"), vec![Seg::Frac("3".into(), "4x".into())]);
    }
}
