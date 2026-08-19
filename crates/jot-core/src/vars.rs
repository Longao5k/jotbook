//! 变量语法 `{{name}}` / `{{name=默认值}}` 的分词与渲染。
//!
//! 关键设计：`{{ }}` 与 Go 模板冲突（`docker inspect -f '{{.Names}}'`、
//! `kubectl -o go-template='{{range ...}}'`）。这里用一条标识符启发式解决：
//! 只有内容是「纯标识符（可带默认值）」的才算变量。`{{.Names}}` 以点开头，
//! `{{range .X}}` 含空格，都会被当成字面量原样保留，用户无需转义。
//! 少数与保留字重名的（`{{end}}`）由保留字表兜住。

/// Go 模板关键字，即使形如标识符也不当变量。
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

/// 内容是否够格当一个变量名。
fn is_var_body(s: &str) -> bool {
    let (name, _) = match s.split_once('=') {
        Some((n, d)) => (n, Some(d)),
        None => (s, None),
    };
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    // 允许 @ 开头的内置变量：@cwd / @git.branch / @env.FOO
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '@') {
        return false;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        return false;
    }
    if RESERVED.contains(&name) {
        return false;
    }
    true
}

/// 把文本切成字面量与变量段。
pub fn tokenize(text: &str) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    let mut lit = String::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        // 转义 \{{ → 字面 {{
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
        // 逐字符推进，保证 UTF-8 边界安全
        let ch = text[i..].chars().next().unwrap();
        lit.push(ch);
        i += ch.len_utf8();
    }

    if !lit.is_empty() {
        out.push(Seg::Lit(lit));
    }
    out
}

/// 文本里出现的变量，按首次出现顺序去重。
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

/// 用取值表渲染最终文本。缺失的变量退回默认值，再缺就原样保留。
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

/// 给列表里显示用的预览：把变量换成 ⟨名字⟩，比一堆花括号好读。
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
        // docker / kubectl 的 Go 模板必须原样保留
        let s = r#"docker ps --format "{{.Names}}\t{{.Status}}""#;
        assert!(refs(s).is_empty(), "Go 模板字段被误判成变量了");
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
}
