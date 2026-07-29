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
    /// `Translators:` comments attached to this string, emitted as `#.` lines.
    pub comments: Vec<String>,
}

const TRANSLATOR_COMMENT_MARK: &str = "Translators";

/// Single or double quoted string pattern fragment
const SQ: &str = r#"'([^'\\]*(?:\\.[^'\\]*)*)'"#;
const DQ: &str = r#""([^"\\]*(?:\\.[^"\\]*)*)""#;
/// Triple-quoted forms, matched ahead of the single-quoted ones.
const TSQ: &str = r#"'''([^\\]*?(?:\\.[^\\]*?)*?)'''"#;
const TDQ: &str = r#""""([^\\]*?(?:\\.[^\\]*?)*?)""""#;

fn str_pattern() -> String {
    format!("(?:{SQ}|{DQ})")
}

/// Python string literal including triple-quoted forms. Triple quotes come
/// first so `"""x"""` is not mis-read as an empty `""` followed by `x`.
fn py_str_pattern() -> String {
    format!("(?s:(?:{TDQ}|{TSQ}|{DQ}|{SQ}))")
}

fn concat_str_pattern() -> String {
    let s = py_str_pattern();
    format!(r"(?:{s}(?:[ \t\n\r\\]+{s})*)")
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

/// `{% trans "msg" %}` / `{% translate "msg" ... context "ctx" %}`.
/// Mirrors Django's inline_re: the message, optional ignored filters, then an
/// optional trailing `context "..."`.
static TEMPLATE_TRANS_RE: LazyLock<Regex> = LazyLock::new(|| {
    let s = str_pattern();
    Regex::new(&format!(
        r#"\{{%\s*(?:trans|translate)\s+{s}(?:[^%]*?\bcontext\s+{s})?[^%]*?%\}}"#
    ))
    .unwrap()
});

static TEMPLATE_BLOCKTRANS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)\{%\s*(?:blocktrans|blocktranslate)((?:\s[^%]*)?)\s*%\}(.*?)\{%\s*(?:endblocktrans|endblocktranslate)\s*%\}"#,
    )
    .unwrap()
});

static TEMPLATE_BLOCKTRANS_PLURAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)\{%\s*(?:blocktrans|blocktranslate)\s+(count\s[^%]*)%\}(.*?)\{%\s*plural\s*%\}(.*?)\{%\s*(?:endblocktrans|endblocktranslate)\s*%\}"#,
    )
    .unwrap()
});

/// Pulls `context "..."` out of a blocktrans tag's argument list.
static TEMPLATE_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let s = str_pattern();
    Regex::new(&format!(r#"\bcontext\s+{s}"#)).unwrap()
});

/// Any `{% ... %}` or `{{ ... }}` construct, used to scope `_()` scanning.
static TEMPLATE_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)\{%.*?%\}|\{\{.*?\}\}"#).unwrap());

/// Opening trans/blocktrans tags, whose strings are handled by dedicated passes.
static TEMPLATE_TRANS_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\{%\s*(?:trans|translate|blocktrans|blocktranslate)\b"#).unwrap()
});

/// `_("...")` constant, mirroring Django's constant_re.
static TEMPLATE_CONSTANT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let s = str_pattern();
    Regex::new(&format!(r#"_\(\s*{s}\s*\)"#)).unwrap()
});

/// Matches {# ... #} inline comments (single-line only, non-greedy)
static TEMPLATE_INLINE_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{#.*?#\}"#).unwrap());

/// Matches {% comment %}...{% endcomment %} block comments (multi-line)
static TEMPLATE_COMMENT_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)\{%\s*comment\s*%\}.*?\{%\s*endcomment\s*%\}"#).unwrap());

/// Converts Django template variables {{ var }} to Python format %(var)s.
/// Django uses the whole VAR token's contents, so filters and dotted lookups
/// are kept verbatim: {{ a.b|filter:1 }} -> %(a.b|filter:1)s
static TEMPLATE_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{\{\s*(.*?)\s*\}\}"#).unwrap());

/// Replace comment regions with whitespace of the same byte length,
/// preserving newlines so that line numbers remain accurate.
fn strip_template_comments(content: &str) -> String {
    let mut result = content.to_string();
    for re in [&*TEMPLATE_COMMENT_BLOCK_RE, &*TEMPLATE_INLINE_COMMENT_RE] {
        let current = result.clone();
        let mut out = String::with_capacity(current.len());
        let mut last_end = 0;
        for m in re.find_iter(&current) {
            out.push_str(&current[last_end..m.start()]);
            for ch in current[m.start()..m.end()].chars() {
                if ch == '\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
            }
            last_end = m.end();
        }
        out.push_str(&current[last_end..]);
        result = out;
    }
    result
}

fn templatize_vars(s: &str) -> String {
    TEMPLATE_VAR_RE.replace_all(s, "%($1)s").to_string()
}

/// Django doubles `%` in TEXT tokens only, never inside a `{{ var }}`.
/// Escape the text spans first, then substitute vars, so a `%` living in a
/// filter argument survives untouched.
fn escape_text_and_templatize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for m in TEMPLATE_VAR_RE.find_iter(s) {
        out.push_str(&escape_lone_percent(&s[last..m.start()]));
        out.push_str(&templatize_vars(m.as_str()));
        last = m.end();
    }
    out.push_str(&escape_lone_percent(&s[last..]));
    out
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
/// Django's trim_whitespace_re is `\s*\n\s*`: only whitespace runs spanning a
/// newline collapse, so repeated spaces within a single line are preserved.
static TRIM_WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\n\s*").unwrap());

fn collapse_whitespace(s: &str) -> String {
    TRIM_WHITESPACE_RE.replace_all(s.trim(), " ").to_string()
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
            let triple = i + 2 < chars.len() && chars[i + 1] == quote && chars[i + 2] == quote;
            i += if triple { 3 } else { 1 };
            let mut s = String::new();
            while i < chars.len() {
                if triple
                    && chars[i] == quote
                    && i + 2 < chars.len()
                    && chars[i + 1] == quote
                    && chars[i + 2] == quote
                {
                    i += 3;
                    break;
                }
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
                } else if chars[i] == quote && !triple {
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

/// Maps a 1-based line number to the `Translators:` comments that xgettext
/// would attach to a string starting on that line: the run of `#` comment
/// lines immediately above it, kept only if one starts with the mark.
fn python_translator_comments(content: &str) -> std::collections::HashMap<usize, Vec<String>> {
    let lines: Vec<&str> = content.lines().collect();
    let mut map = std::collections::HashMap::new();

    for (idx, _) in lines.iter().enumerate() {
        let mut block = Vec::new();
        let mut cursor = idx;
        while cursor > 0 {
            let prev = lines[cursor - 1].trim();
            if let Some(text) = prev.strip_prefix('#') {
                block.push(text.trim().to_string());
                cursor -= 1;
            } else {
                break;
            }
        }
        if block.is_empty() {
            continue;
        }
        block.reverse();
        if let Some(pos) = block
            .iter()
            .position(|l| l.starts_with(TRANSLATOR_COMMENT_MARK))
        {
            map.insert(idx + 1, block[pos..].to_vec());
        }
    }
    map
}

/// `{# Translators: ... #}` and `{% comment %}` blocks, keyed by the line the
/// following template tag starts on.
fn template_translator_comments(content: &str) -> std::collections::HashMap<usize, Vec<String>> {
    let mut map: std::collections::HashMap<usize, Vec<String>> = std::collections::HashMap::new();

    for caps in TEMPLATE_INLINE_COMMENT_RE.captures_iter(content) {
        let m = caps.get(0).unwrap();
        let inner = m
            .as_str()
            .trim_start_matches("{#")
            .trim_end_matches("#}")
            .trim();
        if inner.starts_with(TRANSLATOR_COMMENT_MARK) {
            let line = line_num_at(content, m.start());
            map.entry(line + 1).or_default().push(inner.to_string());
        }
    }

    for caps in TEMPLATE_COMMENT_BLOCK_RE.captures_iter(content) {
        let m = caps.get(0).unwrap();
        let body = m.as_str();
        let inner_start = body.find("%}").map(|i| i + 2).unwrap_or(0);
        let inner_end = body.rfind("{%").unwrap_or(body.len());
        if inner_start >= inner_end {
            continue;
        }
        let inner_lines: Vec<&str> = body[inner_start..inner_end].lines().collect();
        if let Some(pos) = inner_lines
            .iter()
            .rposition(|l| l.trim_start().starts_with(TRANSLATOR_COMMENT_MARK))
        {
            let collected: Vec<String> = inner_lines[pos..]
                .iter()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !collected.is_empty() {
                let end_line = line_num_at(content, m.end());
                map.entry(end_line + 1).or_default().extend(collected);
            }
        }
    }

    map
}

pub fn extract_from_python(content: &str, file_path: &Path) -> Vec<TranslationEntry> {
    // Same per-pattern pass structure as templates; sort back to source order.
    let mut entries: Vec<(usize, TranslationEntry)> = Vec::new();
    let comment_map = python_translator_comments(content);
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
            entries.push((
                m.start(),
                TranslationEntry {
                    msgid,
                    msgid_plural: None,
                    msgctxt: None,
                    references: vec![format!("{file_ref}:{line_num}")],
                    comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                },
            ));
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
                entries.push((
                    m.start(),
                    TranslationEntry {
                        msgid: singular,
                        msgid_plural: Some(plural),
                        msgctxt: None,
                        references: vec![format!("{file_ref}:{line_num}")],
                        comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                    },
                ));
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
                entries.push((
                    m.start(),
                    TranslationEntry {
                        msgid,
                        msgid_plural: None,
                        msgctxt: Some(context),
                        references: vec![format!("{file_ref}:{line_num}")],
                        comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                    },
                ));
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
                entries.push((
                    m.start(),
                    TranslationEntry {
                        msgid: singular,
                        msgid_plural: Some(plural),
                        msgctxt: Some(context),
                        references: vec![format!("{file_ref}:{line_num}")],
                        comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                    },
                ));
            }
        }
    }

    entries.sort_by_key(|(offset, _)| *offset);
    entries.into_iter().map(|(_, e)| e).collect()
}

/// `_("...")` constants inside `{% ... %}` tags and `{{ ... }}` expressions.
/// Django extracts these via constant_re in templatize (covering filter
/// arguments like `{{ x|default:_("y") }}`), but never inside a blocktrans
/// body, where the text is consumed as a literal TEXT token.
fn extract_template_constants(content: &str) -> Vec<(usize, String)> {
    let blocktrans_spans: Vec<(usize, usize)> = TEMPLATE_BLOCKTRANS_RE
        .find_iter(content)
        .map(|m| (m.start(), m.end()))
        .collect();

    let mut found = Vec::new();
    for tag in TEMPLATE_TAG_RE.find_iter(content) {
        // The opening blocktrans/trans tag itself is handled by its own pass.
        if TEMPLATE_TRANS_TAG_RE.is_match(tag.as_str()) {
            continue;
        }
        if blocktrans_spans
            .iter()
            .any(|(s, e)| tag.start() >= *s && tag.end() <= *e)
        {
            continue;
        }
        for caps in TEMPLATE_CONSTANT_RE.captures_iter(tag.as_str()) {
            let m = match caps.get(1).or_else(|| caps.get(2)) {
                Some(m) => m,
                None => continue,
            };
            let msgid = escape_lone_percent(&unescape_string(m.as_str()));
            if !msgid.is_empty() {
                found.push((tag.start(), msgid));
            }
        }
    }
    found
}

fn block_context(args: &str) -> Option<String> {
    let caps = TEMPLATE_CONTEXT_RE.captures(args)?;
    caps.get(1)
        .or_else(|| caps.get(2))
        .map(|m| unescape_string(m.as_str()))
}

pub fn extract_from_template(content: &str, file_path: &Path) -> Vec<TranslationEntry> {
    // Collect translator comments before blanking comment regions out.
    let comment_map = template_translator_comments(content);
    let content = &strip_template_comments(content);
    // Each pattern is scanned in its own pass, so keep the source offset and
    // sort at the end to restore document order (what Django's lexer emits).
    let mut entries: Vec<(usize, TranslationEntry)> = Vec::new();
    let file_ref = file_path.to_string_lossy().to_string();

    for caps in TEMPLATE_TRANS_RE.captures_iter(content) {
        let byte_offset = caps.get(0).unwrap().start();
        let line_num = line_num_at(content, byte_offset);

        let msgid = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| escape_lone_percent(&templatize_vars(&unescape_string(m.as_str()))));

        let msgctxt = caps
            .get(3)
            .or_else(|| caps.get(4))
            .map(|m| unescape_string(m.as_str()));

        if let Some(msgid) = msgid {
            if !msgid.is_empty() {
                entries.push((
                    byte_offset,
                    TranslationEntry {
                        msgid,
                        msgid_plural: None,
                        msgctxt,
                        references: vec![format!("{file_ref}:{line_num}")],
                        comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                    },
                ));
            }
        }
    }

    for caps in TEMPLATE_BLOCKTRANS_PLURAL_RE.captures_iter(content) {
        let byte_offset = caps.get(0).unwrap().start();
        let line_num = line_num_at(content, byte_offset);
        let full = caps.get(0).unwrap().as_str();
        let is_trimmed = full.contains("trimmed");

        // Django only trims when `trimmed` is given; otherwise the body's
        // surrounding whitespace is part of the msgid.
        let process = |s: &str| -> String {
            let v = escape_text_and_templatize(s);
            if is_trimmed {
                collapse_whitespace(&v)
            } else {
                v
            }
        };

        let msgctxt = caps.get(1).and_then(|m| block_context(m.as_str()));
        let singular = caps.get(2).map(|m| process(m.as_str()));
        let plural = caps.get(3).map(|m| process(m.as_str()));

        if let (Some(s), Some(p)) = (singular, plural) {
            if !s.is_empty() {
                entries.push((
                    byte_offset,
                    TranslationEntry {
                        msgid: s,
                        msgid_plural: Some(p),
                        msgctxt,
                        references: vec![format!("{file_ref}:{line_num}")],
                        comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                    },
                ));
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

        let msgctxt = caps.get(1).and_then(|m| block_context(m.as_str()));

        if let Some(m) = caps.get(2) {
            let text = escape_text_and_templatize(m.as_str());
            let text = if is_trimmed {
                collapse_whitespace(&text)
            } else {
                text
            };
            if !text.is_empty() {
                entries.push((
                    byte_offset,
                    TranslationEntry {
                        msgid: text,
                        msgid_plural: None,
                        msgctxt,
                        references: vec![format!("{file_ref}:{line_num}")],
                        comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
                    },
                ));
            }
        }
    }

    for (offset, msgid) in extract_template_constants(content) {
        let line_num = line_num_at(content, offset);
        entries.push((
            offset,
            TranslationEntry {
                msgid,
                msgid_plural: None,
                msgctxt: None,
                references: vec![format!("{file_ref}:{line_num}")],
                comments: comment_map.get(&line_num).cloned().unwrap_or_default(),
            },
        ));
    }

    entries.sort_by_key(|(offset, _)| *offset);
    entries.into_iter().map(|(_, e)| e).collect()
}

pub fn extract_file(file_path: &Path) -> Result<Vec<TranslationEntry>> {
    let content = std::fs::read_to_string(file_path)?;
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // JavaScript uses the same gettext call syntax and quoting as Python, so
    // the Python scanner handles it; only templates need the tag lexer.
    let entries = match ext {
        "py" | "js" | "mjs" | "cjs" | "ts" => extract_from_python(&content, file_path),
        _ => extract_from_template(&content, file_path),
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
        assert_eq!(entries[0].msgid_plural.as_deref(), Some("%(count)d items"));
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
        let html =
            r#"{% blocktrans %}Hello {{ name }}, you have 100% completion{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].msgid,
            "Hello %(name)s, you have 100%% completion"
        );
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

    #[test]
    fn test_template_inline_comment_skipped() {
        let html = r#"{# {% trans "hidden" %} #}
{% trans "visible" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "visible");
    }

    #[test]
    fn test_template_comment_block_skipped() {
        let html = r#"{% comment %}
{% trans "inside comment" %}
{% blocktrans %}Also hidden {{ name }}{% endblocktrans %}
{% endcomment %}
{% trans "after comment" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "after comment");
    }

    #[test]
    fn test_template_comment_preserves_line_numbers() {
        let html = r#"{% comment %}
skip this
{% endcomment %}
{% trans "on line four" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].references[0], "test.html:4");
    }

    #[test]
    fn test_template_mixed_comments_and_content() {
        let html = r#"{% trans "first" %}
{# {% trans "hidden inline" %} #}
{% comment %}{% trans "hidden block" %}{% endcomment %}
{% trans "second" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].msgid, "first");
        assert_eq!(entries[1].msgid, "second");
    }

    #[test]
    fn test_extract_file_routes_email_to_template() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("makemessages_test_email");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("welcome.email");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, r#"{{% load i18n %}}{{% trans "Hello from email" %}}"#).unwrap();
        let entries = extract_file(&file).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Hello from email");
    }

    #[test]
    fn test_extract_file_routes_py_to_python() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("makemessages_test_py");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("views.py");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, r#"_('Python string')"#).unwrap();
        let entries = extract_file(&file).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Python string");
    }

    #[test]
    fn test_blocktrans_inside_comment_block_skipped() {
        let html = r#"{% comment %}
{% blocktrans count counter=items|length %}
{{ counter }} item
{% plural %}
{{ counter }} items
{% endblocktrans %}
{% endcomment %}
{% trans "visible after blocktrans comment" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "visible after blocktrans comment");
    }

    #[test]
    fn test_blocktrans_var_keeps_filters_and_dots() {
        let html =
            r#"{% blocktrans %}Hi {{ user.name }} you have {{ n|add:1 }}{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "Hi %(user.name)s you have %(n|add:1)s");
    }

    #[test]
    fn test_percent_in_filter_arg_not_doubled() {
        let html = r#"{% blocktrans %}50% of {{ x|default:"a%b" }}{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, r#"50%% of %(x|default:"a%b")s"#);
    }

    #[test]
    fn test_trans_with_context() {
        let html = r#"{% translate "File" context "menu" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgctxt.as_deref(), Some("menu"));
        assert_eq!(entries[0].msgid, "File");
    }

    #[test]
    fn test_trans_without_context_has_none() {
        let html = r#"{% translate "File" %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries[0].msgctxt, None);
    }

    #[test]
    fn test_blocktrans_with_context() {
        let html = r#"{% blocktranslate context "email" %}Body{% endblocktranslate %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgctxt.as_deref(), Some("email"));
    }

    #[test]
    fn test_blocktrans_plural_with_context() {
        let html = r#"{% blocktranslate count n=x|length context "cart" %}{{ n }} thing{% plural %}{{ n }} things{% endblocktranslate %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgctxt.as_deref(), Some("cart"));
        assert_eq!(entries[0].msgid_plural.as_deref(), Some("%(n)s things"));
    }

    #[test]
    fn test_constant_in_filter_argument() {
        let html = r#"{{ foo|default:_("fallback") }}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "fallback");
    }

    #[test]
    fn test_constant_in_var_and_block_tag() {
        let html = "{{ _(\"in var\") }}\n{% mytag _(\"in tag\") %}";
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        let ids: Vec<&str> = entries.iter().map(|e| e.msgid.as_str()).collect();
        assert_eq!(ids, vec!["in var", "in tag"]);
    }

    #[test]
    fn test_constant_not_extracted_inside_blocktrans_body() {
        let html = r#"{% blocktrans %}literal _("x") here{% endblocktrans %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, r#"literal _("x") here"#);
    }

    #[test]
    fn test_entries_are_in_source_order() {
        let html = "{% trans \"first\" %}\n{% blocktrans %}second{% endblocktrans %}\n{% trans \"third\" %}";
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        let ids: Vec<&str> = entries.iter().map(|e| e.msgid.as_str()).collect();
        assert_eq!(ids, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_triple_quoted_python_string() {
        let code = "x = _(\"\"\"triple quoted\"\"\")";
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "triple quoted");
    }

    #[test]
    fn test_python_translator_comment() {
        let code = "# Translators: helpful hint\nx = _('Greeting')";
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].comments, vec!["Translators: helpful hint"]);
    }

    #[test]
    fn test_python_non_translator_comment_ignored() {
        let code = "# just a note\nx = _('Greeting')";
        let entries = extract_from_python(code, &PathBuf::from("test.py"));
        assert!(entries[0].comments.is_empty());
    }

    #[test]
    fn test_template_inline_translator_comment() {
        let html = "{# Translators: inline hint #}\n{% trans \"Greeting\" %}";
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].comments, vec!["Translators: inline hint"]);
    }

    #[test]
    fn test_template_block_translator_comment() {
        let html =
            "{% comment %}\nTranslators: block hint\n{% endcomment %}\n{% trans \"Farewell\" %}";
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].comments, vec!["Translators: block hint"]);
    }

    #[test]
    fn test_js_file_uses_python_scanner() {
        use std::io::Write;
        let dir = std::env::temp_dir().join("makemessages_test_js");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.js");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, r#"const a = gettext("JS string");"#).unwrap();
        let entries = extract_file(&file).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].msgid, "JS string");
    }

    #[test]
    fn test_blocktrans_preserves_surrounding_whitespace() {
        let html = r#"{% blocktranslate %} By {{ t }} {% endblocktranslate %}"#;
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries[0].msgid, " By %(t)s ");
    }

    #[test]
    fn test_blocktrans_trimmed_collapses() {
        // Django's trim_whitespace only collapses runs spanning a newline.
        let html = "{% blocktranslate trimmed %}\n   a   {{ x }}\n   b\n{% endblocktranslate %}";
        let entries = extract_from_template(html, &PathBuf::from("test.html"));
        assert_eq!(entries[0].msgid, "a   %(x)s b");
    }
}
