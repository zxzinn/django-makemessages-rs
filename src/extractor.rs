use anyhow::Result;
use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TranslationEntry {
    pub msgid: String,
    pub msgid_plural: Option<String>,
    pub msgctxt: Option<String>,
    pub references: Vec<String>,
}

/// Single or double quoted string pattern fragment
const SQ: &str = r#"'([^'\\]*(?:\\.[^'\\]*)*)'"#;
const DQ: &str = r#""([^"\\]*(?:\\.[^"\\]*)*)""#;

fn str_pattern() -> String {
    format!("(?:{SQ}|{DQ})")
}

fn concat_str_pattern() -> String {
    let s = str_pattern();
    format!(r"(?:{s}(?:[ \t\n]+{s})*)")
}

/// Uses fancy-regex for look-behind to avoid matching obj._() or some_func()
static PYTHON_GETTEXT_RE: LazyLock<FancyRegex> = LazyLock::new(|| {
    let cs = concat_str_pattern();
    FancyRegex::new(&format!(
        r"(?:(?:\b(?:gettext|gettext_lazy|gettext_noop))|(?:(?<![.\w])_))\(\s*{cs}\s*\)"
    ))
    .unwrap()
});

static PYTHON_NGETTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let cs = concat_str_pattern();
    Regex::new(&format!(
        r"\b(?:ngettext|ngettext_lazy)\(\s*{cs}\s*,\s*{cs}\s*,"
    ))
    .unwrap()
});

static PYTHON_PGETTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let cs = concat_str_pattern();
    Regex::new(&format!(
        r"\b(?:pgettext|pgettext_lazy)\(\s*{cs}\s*,\s*{cs}\s*\)"
    ))
    .unwrap()
});

static PYTHON_NPGETTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let cs = concat_str_pattern();
    Regex::new(&format!(
        r"\b(?:npgettext|npgettext_lazy)\(\s*{cs}\s*,\s*{cs}\s*,\s*{cs}\s*,"
    ))
    .unwrap()
});

static TEMPLATE_TRANS_RE: LazyLock<Regex> = LazyLock::new(|| {
    let s = str_pattern();
    Regex::new(&format!(r#"\{{% *(?:trans|translate) +{s}"#)).unwrap()
});

static TEMPLATE_BLOCKTRANS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)\{%\s*(?:blocktrans|blocktranslate)(?:\s[^%]*)?\s*%\}(.*?)\{%\s*(?:endblocktrans|endblocktranslate)\s*%\}"#,
    )
    .unwrap()
});

static TEMPLATE_BLOCKTRANS_PLURAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)\{%\s*(?:blocktrans|blocktranslate)\s+count\s[^%]*%\}(.*?)\{%\s*plural\s*%\}(.*?)\{%\s*(?:endblocktrans|endblocktranslate)\s*%\}"#,
    )
    .unwrap()
});

/// Converts Django template variables {{ var }} to Python format %(var)s
static TEMPLATE_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{\{\s*(\w+)\s*\}\}"#).unwrap());

fn templatize_vars(s: &str) -> String {
    TEMPLATE_VAR_RE.replace_all(s, "%($1)s").to_string()
}

/// Escape lone `%` that aren't part of `%(name)s` format strings.
/// In PO files, a literal `%` must be written as `%%`.
fn escape_lone_percent(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            if i + 1 < chars.len() && chars[i + 1] == '(' {
                result.push('%');
            } else if i + 1 < chars.len() && chars[i + 1] == '%' {
                result.push_str("%%");
                i += 2;
                continue;
            } else {
                result.push_str("%%");
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    result
}

/// Collapse whitespace like Django's `trimmed` option on blocktrans.
fn collapse_whitespace(s: &str) -> String {
    let trimmed = s.trim();
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(trimmed, " ").to_string()
}

fn unescape_string(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\")
        .replace("\\'", "'")
        .replace("\\\"", "\"")
}

fn extract_concat_from_text(text: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' || ch == '"' {
            let quote = ch;
            i += 1;
            let mut s = String::new();
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    let escaped = chars[i + 1];
                    match escaped {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        '\'' => s.push('\''),
                        '"' => s.push('"'),
                        _ => {
                            s.push('\\');
                            s.push(escaped);
                        }
                    }
                    i += 2;
                } else if chars[i] == quote {
                    i += 1;
                    break;
                } else {
                    s.push(chars[i]);
                    i += 1;
                }
            }
            result.push_str(&s);
        } else {
            i += 1;
        }
    }

    result
}

fn split_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut in_sq = false;
    let mut in_dq = false;
    let mut escape = false;
    let mut current_start = 0;

    for (i, ch) in text.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if ch == '\'' && !in_dq {
            in_sq = !in_sq;
        } else if ch == '"' && !in_sq {
            in_dq = !in_dq;
        } else if !in_sq && !in_dq {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
            } else if ch == ',' && depth == 0 {
                args.push(text[current_start..i].trim().to_string());
                current_start = i + 1;
            }
        }
    }
    let last = text[current_start..].trim().to_string();
    if !last.is_empty() {
        args.push(last);
    }
    args
}

fn line_num_at(content: &str, byte_offset: usize) -> usize {
    content[..byte_offset].matches('\n').count() + 1
}

pub fn extract_from_python(content: &str, file_path: &Path) -> Vec<TranslationEntry> {
    let mut entries = Vec::new();
    let file_ref = file_path.to_string_lossy().to_string();

    for m in PYTHON_GETTEXT_RE.find_iter(content) {
        let m = match m {
            Ok(m) => m,
            Err(_) => continue,
        };
        let line_num = line_num_at(content, m.start());
        let matched = m.as_str();
        let paren_start = matched.find('(').unwrap();
        let inner = &matched[paren_start + 1..matched.len() - 1].trim();
        let msgid = extract_concat_from_text(inner);
        if !msgid.is_empty() {
            entries.push(TranslationEntry {
                msgid,
                msgid_plural: None,
                msgctxt: None,
                references: vec![format!("{file_ref}:{line_num}")],
            });
        }
    }

    for m in PYTHON_NGETTEXT_RE.find_iter(content) {
        let line_num = line_num_at(content, m.start());
        let matched = m.as_str();
        let paren_start = matched.find('(').unwrap();
        let inner = &matched[paren_start + 1..];
        let args = split_args(inner);
        if args.len() >= 2 {
            let singular = extract_concat_from_text(&args[0]);
            let plural = extract_concat_from_text(&args[1]);
            if !singular.is_empty() && !plural.is_empty() {
                entries.push(TranslationEntry {
                    msgid: singular,
                    msgid_plural: Some(plural),
                    msgctxt: None,
                    references: vec![format!("{file_ref}:{line_num}")],
                });
            }
        }
    }

    for m in PYTHON_PGETTEXT_RE.find_iter(content) {
        let line_num = line_num_at(content, m.start());
        let matched = m.as_str();
        let paren_start = matched.find('(').unwrap();
        let inner = &matched[paren_start + 1..matched.len() - 1];
        let args = split_args(inner);
        if args.len() >= 2 {
            let context = extract_concat_from_text(&args[0]);
            let msgid = extract_concat_from_text(&args[1]);
            if !msgid.is_empty() {
                entries.push(TranslationEntry {
                    msgid,
                    msgid_plural: None,
                    msgctxt: Some(context),
                    references: vec![format!("{file_ref}:{line_num}")],
                });
            }
        }
    }

    for m in PYTHON_NPGETTEXT_RE.find_iter(content) {
        let line_num = line_num_at(content, m.start());
        let matched = m.as_str();
        let paren_start = matched.find('(').unwrap();
        let inner = &matched[paren_start + 1..];
        let args = split_args(inner);
        if args.len() >= 3 {
            let context = extract_concat_from_text(&args[0]);
            let singular = extract_concat_from_text(&args[1]);
            let plural = extract_concat_from_text(&args[2]);
            if !singular.is_empty() && !plural.is_empty() {
                entries.push(TranslationEntry {
                    msgid: singular,
                    msgid_plural: Some(plural),
                    msgctxt: Some(context),
                    references: vec![format!("{file_ref}:{line_num}")],
                });
            }
        }
    }

    entries
}

pub fn extract_from_template(content: &str, file_path: &Path) -> Vec<TranslationEntry> {
    let mut entries = Vec::new();
    let file_ref = file_path.to_string_lossy().to_string();

    for caps in TEMPLATE_TRANS_RE.captures_iter(content) {
        let byte_offset = caps.get(0).unwrap().start();
        let line_num = line_num_at(content, byte_offset);

        let msgid = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| escape_lone_percent(&templatize_vars(&unescape_string(m.as_str()))));

        if let Some(msgid) = msgid {
            if !msgid.is_empty() {
                entries.push(TranslationEntry {
                    msgid,
                    msgid_plural: None,
                    msgctxt: None,
                    references: vec![format!("{file_ref}:{line_num}")],
                });
            }
        }
    }

    for caps in TEMPLATE_BLOCKTRANS_PLURAL_RE.captures_iter(content) {
        let byte_offset = caps.get(0).unwrap().start();
        let line_num = line_num_at(content, byte_offset);
        let full = caps.get(0).unwrap().as_str();
        let is_trimmed = full.contains("trimmed");

        let process = |s: &str| -> String {
            let v = templatize_vars(s.trim());
            let v = if is_trimmed { collapse_whitespace(&v) } else { v };
            escape_lone_percent(&v)
        };

        let singular = caps.get(1).map(|m| process(m.as_str()));
        let plural = caps.get(2).map(|m| process(m.as_str()));

        if let (Some(s), Some(p)) = (singular, plural) {
            if !s.is_empty() {
                entries.push(TranslationEntry {
                    msgid: s,
                    msgid_plural: Some(p),
                    msgctxt: None,
                    references: vec![format!("{file_ref}:{line_num}")],
                });
            }
        }
    }

    let plural_tag_re = Regex::new(r#"\{%\s*plural\s*%\}"#).unwrap();

    for caps in TEMPLATE_BLOCKTRANS_RE.captures_iter(content) {
        let full_match = caps.get(0).unwrap().as_str();
        if plural_tag_re.is_match(full_match) {
            continue;
        }

        let byte_offset = caps.get(0).unwrap().start();
        let line_num = line_num_at(content, byte_offset);
        let is_trimmed = full_match.contains("trimmed");

        if let Some(m) = caps.get(1) {
            let text = templatize_vars(m.as_str().trim());
            let text = if is_trimmed { collapse_whitespace(&text) } else { text };
            let text = escape_lone_percent(&text);
            if !text.is_empty() {
                entries.push(TranslationEntry {
                    msgid: text,
                    msgid_plural: None,
                    msgctxt: None,
                    references: vec![format!("{file_ref}:{line_num}")],
                });
            }
        }
    }

    entries
}

pub fn extract_file(file_path: &Path) -> Result<Vec<TranslationEntry>> {
    let content = std::fs::read_to_string(file_path)?;
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let entries = match ext {
        "py" => extract_from_python(&content, file_path),
        "html" | "txt" => extract_from_template(&content, file_path),
        _ => Vec::new(),
    };

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extract_gettext() {
        let code = r#"
from django.utils.translation import gettext_lazy as _

msg = _('Hello world')
msg2 = gettext_lazy("File format not supported")
"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].msgid, "Hello world");
        assert_eq!(entries[1].msgid, "File format not supported");
    }

    #[test]
    fn test_extract_pgettext() {
        let code = r#"pgettext_lazy('menu', 'File')"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgctxt.as_deref(), Some("menu"));
        assert_eq!(entries[0].msgid, "File");
    }

    #[test]
    fn test_extract_ngettext() {
        let code = r#"ngettext_lazy('%(count)d item', '%(count)d items', count)"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "%(count)d item");
        assert_eq!(
            entries[0].msgid_plural.as_deref(),
            Some("%(count)d items")
        );
    }

    #[test]
    fn test_extract_template_trans() {
        let html = r#"{% trans "Welcome" %} and {% trans 'Goodbye' %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].msgid, "Welcome");
        assert_eq!(entries[1].msgid, "Goodbye");
    }

    #[test]
    fn test_extract_template_translate() {
        let html = r#"{% translate "Welcome" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Welcome");
    }

    #[test]
    fn test_extract_template_blocktrans() {
        let html = r#"{% blocktrans %}Hello {{ name }}{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Hello %(name)s");
    }

    #[test]
    fn test_extract_template_blocktrans_plural() {
        let html = r#"{% blocktrans count counter=list|length %}{{ counter }} item selected{% plural %}{{ counter }} items selected{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "%(counter)s item selected");
        assert_eq!(
            entries[0].msgid_plural.as_deref(),
            Some("%(counter)s items selected")
        );
    }

    #[test]
    fn test_extract_implicit_concat() {
        let code = r#"_(
            'At least one of knowledge_base_ids, knowledge_base_file_ids, '
            'or chatbot_file_ids parameter is required.'
        )"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].msgid,
            "At least one of knowledge_base_ids, knowledge_base_file_ids, or chatbot_file_ids parameter is required."
        );
    }

    #[test]
    fn test_underscore_not_in_method() {
        let code = r#"obj._('hello')"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_underscore_not_in_word() {
        let code = r#"some_func('hello')"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_template_percent_escape() {
        let html = r#"{% trans "Error Rate (%)" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Error Rate (%%)");
    }

    #[test]
    fn test_template_var_not_double_escaped() {
        let html = r#"{% blocktrans %}Hello {{ name }}, you have 100% completion{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Hello %(name)s, you have 100%% completion");
    }

    #[test]
    fn test_template_blocktranslate_trimmed() {
        let html = r#"{% blocktranslate trimmed %}
          Or, <a href="{{ signup_url }}">sign up</a>
          for a {{ site_name }} account and sign in below:
        {% endblocktranslate %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].msgid,
            "Or, <a href=\"%(signup_url)s\">sign up</a> for a %(site_name)s account and sign in below:"
        );
    }

    #[test]
    fn test_python_escaped_quotes_in_single_quoted() {
        let code = r#"_('Celery queue "%(queue)s" purged successfully.')"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].msgid,
            "Celery queue \"%(queue)s\" purged successfully."
        );
    }

    #[test]
    fn test_python_percent_formatting_after_call() {
        let code = r#"_('Queue "%(queue)s" purged.') % {'queue': name}"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Queue \"%(queue)s\" purged.");
    }

    #[test]
    fn test_same_line_implicit_concat() {
        let code = r#"_('Can not modify maigpt chatbot. please modify ' 'Config.maigpt_settings in admin.')"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].msgid,
            "Can not modify maigpt chatbot. please modify Config.maigpt_settings in admin."
        );
    }

    #[test]
    fn test_gettext_noop() {
        let code = r#"gettext_noop('Draft')"#;
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Draft");
    }
}
