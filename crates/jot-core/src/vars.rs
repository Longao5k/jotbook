//! Tokenizing and rendering the `{{name}}` / `{{name=default}}` syntax.
//!
//! The key design point: `{{ }}` collides with Go templates
//! (`docker inspect -f '{{.Names}}'`, `kubectl -o go-template='{{range ...}}'`).
//! An identifier heuristic settles it: only a plain identifier (optionally with
//! a default) counts. `{{.Names}}` starts with a dot and `{{range .X}}` has a
//! space, so both stay literal with no escaping. `{{end}}` is caught by a reserved list.

/// Go template keywords, never treated as variables even though they look like identifiers.
const RESERVED: &[&str] = &[
    "end", "else", "if", "range", "with", "template", "block", "define", "printf", "print",
    "println", "break", "continue",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Seg {
    Lit(String),
    Var {
        name: String,
        default: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarRef {
    pub name: String,
    pub default: Option<String>,
}

/// Does this content qualify as a variable name?
fn is_var_body(s: &str) -> bool {
    let (name, _) = match s.split_once('=') {
        Some((n, d)) => (n, Some(d)),
        None => (s, None),
    };
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    // Allow @-prefixed built-ins: @cwd / @git.branch / @env.FOO
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '@') {
        return false;
    }
    // Hyphens are everywhere in real names (my-api-service, my-host), and
    // allowing them here is safe: the first character still has to be a
    // letter, so Go's whitespace-trim `{{- if}}` is untouched.
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-') {
        return false;
    }
    if RESERVED.contains(&name) {
        return false;
    }
    true
}

/// Split text into literal and variable segments.
pub fn tokenize(text: &str) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    let mut lit = String::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        // Escape: \{{ becomes a literal {{
        if bytes[i] == b'\\' && text[i..].starts_with("\\{{") {
            lit.push_str("{{");
            i += 3;
            continue;
        }
        if text[i..].starts_with("{{") {
            if let Some(rel) = text[i + 2..].find("}}") {
                let body = &text[i + 2..i + 2 + rel];
                if is_var_body(body) {
                    if !lit.is_empty() {
                        out.push(Seg::Lit(std::mem::take(&mut lit)));
                    }
                    let (name, default) = match body.split_once('=') {
                        Some((n, d)) => (n.trim().to_string(), Some(d.to_string())),
                        None => (body.trim().to_string(), None),
                    };
                    out.push(Seg::Var { name, default });
                    i = i + 2 + rel + 2;
                    continue;
                }
            }
        }
        // Advance one character at a time to stay on UTF-8 boundaries
        let ch = text[i..].chars().next().unwrap();
        lit.push(ch);
        i += ch.len_utf8();
    }

    if !lit.is_empty() {
        out.push(Seg::Lit(lit));
    }
    out
}

/// Variables appearing in the text, deduplicated in order of first appearance.
pub fn refs(text: &str) -> Vec<VarRef> {
    let mut seen: Vec<VarRef> = Vec::new();
    for seg in tokenize(text) {
        if let Seg::Var { name, default } = seg {
            if !seen.iter().any(|v| v.name == name) {
                seen.push(VarRef { name, default });
            }
        }
    }
    seen
}

pub fn has_vars(text: &str) -> bool {
    tokenize(text).iter().any(|s| matches!(s, Seg::Var { .. }))
}

/// Render with a value table. Missing variables fall back to their default, then to themselves.
pub fn render(text: &str, values: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    for seg in tokenize(text) {
        match seg {
            Seg::Lit(s) => out.push_str(&s),
            Seg::Var { name, default } => match values.get(&name) {
                Some(v) => out.push_str(v),
                None => match default {
                    Some(d) => out.push_str(&d),
                    None => {
                        out.push_str("{{");
                        out.push_str(&name);
                        out.push_str("}}");
                    }
                },
            },
        }
    }
    out
}

/// Preview for a listing, with anything already settled filled in.
///
/// A listing that shows `⟨service⟩` for a value the profile already knows
/// reads as "this is not usable yet", when in fact selecting it produces the
/// finished command. Show what is settled, and keep the placeholder only for
/// what will actually be asked.
pub fn preview_with(text: &str, values: &std::collections::HashMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    for seg in tokenize(text) {
        match seg {
            Seg::Lit(s) => out.push_str(&s),
            Seg::Var { name, default } => match values.get(&name).or(default.as_ref()) {
                Some(v) => out.push_str(v),
                None => {
                    out.push('⟨');
                    out.push_str(name.trim_start_matches('@'));
                    out.push('⟩');
                }
            },
        }
    }
    out
}

/// Preview for the list: variables become ⟨name⟩, which reads better than braces.
pub fn preview(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for seg in tokenize(text) {
        match seg {
            Seg::Lit(s) => out.push_str(&s),
            Seg::Var { name, .. } => {
                out.push('⟨');
                out.push_str(name.trim_start_matches('@'));
                out.push('⟩');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn plain_variable() {
        assert_eq!(
            refs("systemctl restart {{service}}"),
            vec![VarRef {
                name: "service".into(),
                default: None
            }]
        );
    }

    #[test]
    fn inline_default() {
        let r = refs("python -m http.server {{port=8000}}");
        assert_eq!(r[0].name, "port");
        assert_eq!(r[0].default.as_deref(), Some("8000"));
    }

    #[test]
    fn go_template_is_not_a_variable() {
        // Go templates from docker and kubectl must survive untouched
        let s = r#"docker ps --format "{{.Names}}\t{{.Status}}""#;
        assert!(
            refs(s).is_empty(),
            "a Go template field was mistaken for a variable"
        );
        assert_eq!(render(s, &HashMap::new()), s);
    }

    #[test]
    fn go_template_keywords_are_reserved() {
        let s = "{{range .Networks}}{{.IPAddress}}{{end}}";
        assert!(refs(s).is_empty());
    }

    #[test]
    fn escape_sequence() {
        assert_eq!(render(r"\{{literal}}", &HashMap::new()), "{{literal}}");
    }

    #[test]
    fn render_substitutes() {
        let mut v = HashMap::new();
        v.insert("service".to_string(), "api.service".to_string());
        assert_eq!(
            render("sudo systemctl restart {{service}}", &v),
            "sudo systemctl restart api.service"
        );
    }

    /// Regression: hyphenated names were silently not variables at all, so
    /// `{{my-api-service}}` stayed as literal braces in the command.
    #[test]
    fn hyphens_are_allowed_in_names() {
        let r = refs("sudo systemctl restart {{my-api-service}}");
        assert_eq!(r.len(), 1, "a hyphenated name was not recognised");
        assert_eq!(r[0].name, "my-api-service");

        let mut v = HashMap::new();
        v.insert("my-api-service".to_string(), "api.service".to_string());
        assert_eq!(
            render("sudo systemctl restart {{my-api-service}}", &v),
            "sudo systemctl restart api.service"
        );
    }

    /// Go's whitespace-trim marker must still not be mistaken for a variable.
    #[test]
    fn go_trim_markers_are_not_variables() {
        assert!(refs("{{- if .X}}y{{- end}}").is_empty());
    }

    #[test]
    fn builtin_names_allowed() {
        let r = refs("echo {{@cwd}} {{@git.branch}}");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name, "@cwd");
    }

    #[test]
    fn cjk_is_utf8_safe() {
        let s = "echo 中文 {{name}} 结尾";
        let mut v = HashMap::new();
        v.insert("name".to_string(), "值".to_string());
        assert_eq!(render(s, &v), "echo 中文 值 结尾");
    }

    #[test]
    fn preview_reads_better() {
        assert_eq!(preview("restart {{service}}"), "restart ⟨service⟩");
    }

    /// A listing should not imply an entry is unusable when the profile has
    /// already settled the value.
    #[test]
    fn preview_fills_in_what_is_already_known() {
        let mut v = HashMap::new();
        v.insert("service".to_string(), "api.service".to_string());
        assert_eq!(
            preview_with("restart {{service}} on {{host}}", &v),
            "restart api.service on ⟨host⟩"
        );
        // An inline default counts as settled too
        assert_eq!(
            preview_with("serve {{port=8000}}", &HashMap::new()),
            "serve 8000"
        );
    }
}
