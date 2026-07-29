use crate::extractor::TranslationEntry;
use anyhow::Result;
use indexmap::IndexMap;
use regex::Regex;
use std::fmt::Write as FmtWrite;
use std::path::Path;
use std::sync::LazyLock;

static PO_HEADER_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?ms)^msgid\s+""\nmsgstr\s+"[^"]*"(?:\n\s*"[^"]*")*"#).unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocationMode {
    #[default]
    Full,
    File,
    Never,
}

#[derive(Debug, Clone)]
pub(crate) struct PoEntry {
    comments: Vec<String>,
    references: Vec<String>,
    flags: Vec<String>,
    msgctxt: Option<String>,
    msgid: String,
    msgid_plural: Option<String>,
    msgstr: Vec<String>,
    /// Entry no longer present in the source, written back as `#~` lines.
    obsolete: bool,
}

fn entry_key(msgctxt: &Option<String>, msgid: &str) -> String {
    match msgctxt {
        Some(ctx) => format!("{ctx}\x04{msgid}"),
        None => msgid.to_string(),
    }
}

fn escape_po_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn format_po_string(key: &str, value: &str) -> String {
    let escaped = escape_po_string(value);
    if escaped.contains("\\n") && escaped != "\\n" {
        let parts: Vec<&str> = escaped.split("\\n").collect();
        let mut result = format!("{key} \"\"\n");
        for (i, part) in parts.iter().enumerate() {
            if i < parts.len() - 1 {
                let _ = writeln!(result, "\"{part}\\n\"");
            } else if !part.is_empty() {
                let _ = writeln!(result, "\"{part}\"");
            }
        }
        result.trim_end().to_string()
    } else {
        format!("{key} \"{escaped}\"")
    }
}

fn parse_po_string(raw: &str) -> String {
    let mut result = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(inner) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            result.push_str(inner);
        }
    }
    result
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

pub(crate) fn parse_po_file(content: &str) -> IndexMap<String, PoEntry> {
    let mut entries = IndexMap::new();

    // Obsolete blocks are ordinary entries behind a `#~` prefix; unwrap them
    // so the main loop can parse them, and remember which lines were marked.
    let raw_lines: Vec<&str> = content.lines().collect();
    let mut obsolete_flags = Vec::with_capacity(raw_lines.len());
    let unwrapped: Vec<String> = raw_lines
        .iter()
        .map(|l| {
            let t = l.trim_start();
            if let Some(rest) = t.strip_prefix("#~") {
                obsolete_flags.push(true);
                rest.trim_start().to_string()
            } else {
                obsolete_flags.push(false);
                (*l).to_string()
            }
        })
        .collect();
    let lines: Vec<&str> = unwrapped.iter().map(|s| s.as_str()).collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty()
            || (line.starts_with('#')
                && !line.starts_with("#:")
                && !line.starts_with("#,")
                && !line.starts_with("#."))
        {
            if line.starts_with('#') {
                i += 1;
                continue;
            }
            i += 1;
            continue;
        }

        let entry_start = i;
        let mut comments = Vec::new();
        let mut references = Vec::new();
        let mut flags = Vec::new();

        while i < lines.len() {
            let l = lines[i].trim();
            if l.starts_with("#.") {
                comments.push(l.to_string());
                i += 1;
            } else if l.starts_with("#:") {
                let refs = l[2..].trim();
                for r in refs.split_whitespace() {
                    references.push(r.to_string());
                }
                i += 1;
            } else if l.starts_with("#,") {
                let f = l[2..].trim();
                for flag in f.split(',') {
                    let flag = flag.trim();
                    if !flag.is_empty() {
                        flags.push(flag.to_string());
                    }
                }
                i += 1;
            } else if l.starts_with("#|") || l.starts_with("# ") || l == "#" {
                i += 1;
            } else {
                break;
            }
        }

        let mut msgctxt = None;
        if i < lines.len() && lines[i].trim().starts_with("msgctxt ") {
            let mut raw = lines[i]
                .trim()
                .strip_prefix("msgctxt ")
                .unwrap_or("")
                .to_string();
            i += 1;
            while i < lines.len() && lines[i].trim().starts_with('"') {
                raw.push('\n');
                raw.push_str(lines[i].trim());
                i += 1;
            }
            msgctxt = Some(parse_po_string(&raw));
        }

        if i >= lines.len() || !lines[i].trim().starts_with("msgid ") {
            i += 1;
            continue;
        }

        let mut msgid_raw = lines[i]
            .trim()
            .strip_prefix("msgid ")
            .unwrap_or("")
            .to_string();
        i += 1;
        while i < lines.len()
            && lines[i].trim().starts_with('"')
            && !lines[i].trim().starts_with("\"\"")
            || (i < lines.len()
                && lines[i].trim().starts_with('"')
                && !lines[i].trim().starts_with("msgid_plural")
                && !lines[i].trim().starts_with("msgstr"))
        {
            if lines[i].trim().starts_with("msgid_plural") || lines[i].trim().starts_with("msgstr")
            {
                break;
            }
            msgid_raw.push('\n');
            msgid_raw.push_str(lines[i].trim());
            i += 1;
        }
        let msgid = parse_po_string(&msgid_raw);

        let mut msgid_plural = None;
        if i < lines.len() && lines[i].trim().starts_with("msgid_plural ") {
            let mut raw = lines[i]
                .trim()
                .strip_prefix("msgid_plural ")
                .unwrap_or("")
                .to_string();
            i += 1;
            while i < lines.len()
                && lines[i].trim().starts_with('"')
                && !lines[i].trim().starts_with("msgstr")
            {
                raw.push('\n');
                raw.push_str(lines[i].trim());
                i += 1;
            }
            msgid_plural = Some(parse_po_string(&raw));
        }

        let mut msgstr_list = Vec::new();
        while i < lines.len() && lines[i].trim().starts_with("msgstr") {
            let line_content = lines[i].trim();
            let value_part = if line_content.starts_with("msgstr[") {
                line_content
                    .splitn(2, ']')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string()
            } else {
                line_content
                    .strip_prefix("msgstr ")
                    .unwrap_or("")
                    .to_string()
            };
            let mut raw = value_part;
            i += 1;
            while i < lines.len()
                && lines[i].trim().starts_with('"')
                && !lines[i].trim().starts_with("msgid")
                && !lines[i].trim().starts_with("msgctxt")
                && !lines[i].trim().starts_with("msgstr")
                && !lines[i].trim().starts_with('#')
            {
                raw.push('\n');
                raw.push_str(lines[i].trim());
                i += 1;
            }
            msgstr_list.push(parse_po_string(&raw));
        }

        if msgstr_list.is_empty() {
            msgstr_list.push(String::new());
        }

        if msgid.is_empty() && msgctxt.is_none() {
            i += 1;
            continue;
        }

        let key = entry_key(&msgctxt, &msgid);
        entries.insert(
            key,
            PoEntry {
                comments,
                references,
                flags,
                msgctxt,
                msgid,
                msgid_plural,
                msgstr: msgstr_list,
                obsolete: obsolete_flags.get(entry_start).copied().unwrap_or(false),
            },
        );
    }

    entries
}

fn format_entry(
    entry: &PoEntry,
    location_mode: LocationMode,
    no_flags: bool,
    sort_output: bool,
    no_wrap: bool,
) -> String {
    let mut lines = Vec::new();

    for comment in &entry.comments {
        lines.push(comment.clone());
    }

    if location_mode != LocationMode::Never && !entry.references.is_empty() && !entry.obsolete {
        let mut refs = entry.references.clone();
        if location_mode == LocationMode::File {
            refs = refs
                .iter()
                .map(|r| {
                    r.rsplit_once(':')
                        .map_or(r.as_str(), |(file, _)| file)
                        .to_string()
                })
                .collect();
            refs.dedup();
        }
        if sort_output {
            refs.sort();
        }
        if no_wrap {
            lines.push(format!("#: {}", refs.join(" ")));
        } else {
            let mut line = String::from("#:");
            for r in &refs {
                if line.len() > 2 && line.len() + 1 + r.len() >= 80 {
                    lines.push(line);
                    line = String::from("#:");
                }
                line.push(' ');
                line.push_str(r);
            }
            lines.push(line);
        }
    }

    if !no_flags && !entry.flags.is_empty() {
        lines.push(format!("#, {}", entry.flags.join(", ")));
    }

    if let Some(ctx) = &entry.msgctxt {
        lines.push(format_po_string("msgctxt", ctx));
    }

    lines.push(format_po_string("msgid", &entry.msgid));

    if let Some(plural) = &entry.msgid_plural {
        lines.push(format_po_string("msgid_plural", plural));
        for (i, msgstr) in entry.msgstr.iter().enumerate() {
            lines.push(format_po_string(&format!("msgstr[{i}]"), msgstr));
        }
    } else {
        let msgstr = entry.msgstr.first().map(|s| s.as_str()).unwrap_or("");
        lines.push(format_po_string("msgstr", msgstr));
    }

    if entry.obsolete {
        // gettext marks every line of a commented-out entry, continuations too.
        return lines
            .iter()
            .flat_map(|l| l.lines())
            .map(|l| format!("#~ {l}"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    lines.join("\n")
}

pub struct PoFileOptions {
    pub location_mode: LocationMode,
    pub no_obsolete: bool,
    pub no_wrap: bool,
    pub sort_output: bool,
    #[allow(dead_code)]
    pub no_fuzzy_matching: bool,
    pub no_flags: bool,
    pub keep_header: bool,
}

fn extracted_comments(entry: &TranslationEntry) -> Vec<String> {
    entry.comments.iter().map(|c| format!("#. {c}")).collect()
}

pub fn merge_entries(
    extracted: &[TranslationEntry],
    existing_content: Option<&str>,
    locale: &str,
    options: &PoFileOptions,
) -> String {
    let existing_entries = existing_content
        .map(|c| parse_po_file(c))
        .unwrap_or_default();

    let existing_header_with_comments = existing_content.and_then(|c| {
        if options.keep_header {
            PO_HEADER_PATTERN.find(c).map(|m| {
                let header_str = m.as_str().to_string();
                let before_header = &c[..m.start()];
                let comment_lines: String = before_header
                    .lines()
                    .filter(|l| l.starts_with('#'))
                    .map(|l| format!("{l}\n"))
                    .collect();
                format!("{comment_lines}{header_str}")
            })
        } else {
            None
        }
    });

    let mut new_entries: IndexMap<String, PoEntry> = IndexMap::new();

    for entry in extracted {
        let key = entry_key(&entry.msgctxt, &entry.msgid);

        if let Some(existing) = new_entries.get_mut(&key) {
            existing.references.extend(entry.references.clone());
            for c in extracted_comments(entry) {
                if !existing.comments.contains(&c) {
                    existing.comments.push(c);
                }
            }
        } else if let Some(existing) = existing_entries.get(&key) {
            let mut merged = existing.clone();
            merged.references = entry.references.clone();
            merged.flags.retain(|f| f != "fuzzy");
            // Comments are re-derived from source every run, so replace rather
            // than keep whatever the previous run left behind.
            merged.comments = extracted_comments(entry);
            new_entries.insert(key, merged);
        } else {
            new_entries.insert(
                key,
                PoEntry {
                    comments: extracted_comments(entry),
                    references: entry.references.clone(),
                    flags: Vec::new(),
                    msgctxt: entry.msgctxt.clone(),
                    msgid: entry.msgid.clone(),
                    msgid_plural: entry.msgid_plural.clone(),
                    msgstr: if entry.msgid_plural.is_some() {
                        vec![String::new(), String::new()]
                    } else {
                        vec![String::new()]
                    },
                    obsolete: false,
                },
            );
        }
    }

    if options.sort_output {
        new_entries.sort_keys();
    }

    // Entries that vanished from the source are kept as `#~` blocks so the
    // translations survive, unless --no-obsolete asks for them to be dropped.
    let mut obsolete_entries: Vec<PoEntry> = Vec::new();
    if !options.no_obsolete {
        for (key, existing) in &existing_entries {
            if new_entries.contains_key(key) {
                continue;
            }
            // A previously-obsolete entry with no translation is dead weight.
            if existing.msgstr.iter().all(|s| s.is_empty()) {
                continue;
            }
            let mut e = existing.clone();
            e.obsolete = true;
            e.references.clear();
            obsolete_entries.push(e);
        }
        if options.sort_output {
            obsolete_entries.sort_by(|a, b| {
                entry_key(&a.msgctxt, &a.msgid).cmp(&entry_key(&b.msgctxt, &b.msgid))
            });
        }
    }

    let header = if let Some(h) = existing_header_with_comments {
        h
    } else {
        generate_header(locale)
    };

    let mut output = String::new();
    output.push_str(&header);
    output.push_str("\n\n");

    for (_, entry) in &new_entries {
        output.push_str(&format_entry(
            entry,
            options.location_mode,
            options.no_flags,
            options.sort_output,
            options.no_wrap,
        ));
        output.push_str("\n\n");
    }

    for entry in &obsolete_entries {
        output.push_str(&format_entry(
            entry,
            options.location_mode,
            options.no_flags,
            options.sort_output,
            options.no_wrap,
        ));
        output.push_str("\n\n");
    }

    output.trim_end().to_string()
}

fn generate_header(locale: &str) -> String {
    format!(
        r#"msgid ""
msgstr ""
"Project-Id-Version: PACKAGE VERSION\n"
"Report-Msgid-Bugs-To: \n"
"POT-Creation-Date: 2024-01-01 00:00+0000\n"
"PO-Revision-Date: YEAR-MO-DA HO:MI+ZONE\n"
"Last-Translator: FULL NAME <EMAIL@ADDRESS>\n"
"Language-Team: LANGUAGE <LL@li.org>\n"
"Language: {locale}\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n""#
    )
}

pub fn write_po_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{content}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_po() {
        let content = r#"
msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"

#: views.py:10
msgid "Hello"
msgstr "你好"

#: views.py:20
msgid "Goodbye"
msgstr ""
"#;
        let entries = parse_po_file(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries["Hello"].msgstr[0], "你好");
        assert!(entries["Goodbye"].msgstr[0].is_empty());
    }

    #[test]
    fn test_merge_preserves_translations() {
        let existing = r#"msgid ""
msgstr ""
"Language: zh_Hant\n"

#: old.py:1
msgid "Hello"
msgstr "你好"
"#;
        let extracted = vec![TranslationEntry {
            msgid: "Hello".to_string(),
            msgid_plural: None,
            msgctxt: None,
            references: vec!["new.py:5".to_string()],
            comments: Vec::new(),
        }];
        let options = PoFileOptions {
            location_mode: LocationMode::Full,
            no_obsolete: true,
            no_wrap: true,
            sort_output: true,
            no_fuzzy_matching: true,
            no_flags: false,
            keep_header: true,
        };
        let result = merge_entries(&extracted, Some(existing), "zh_Hant", &options);
        assert!(result.contains("你好"));
        assert!(result.contains("new.py:5"));
        assert!(!result.contains("old.py:1"));
    }

    #[test]
    fn test_format_po_string_simple() {
        assert_eq!(format_po_string("msgid", "Hello"), r#"msgid "Hello""#);
    }

    fn make_entry(refs: Vec<&str>) -> PoEntry {
        PoEntry {
            comments: Vec::new(),
            references: refs.into_iter().map(String::from).collect(),
            flags: Vec::new(),
            msgctxt: None,
            msgid: "x".to_string(),
            msgid_plural: None,
            msgstr: vec![String::new()],
            obsolete: false,
        }
    }

    #[test]
    fn test_wrap_references_short_fits_one_line() {
        let entry = make_entry(vec!["a.py:1", "b.py:2"]);
        let out = format_entry(&entry, LocationMode::Full, false, false, false);
        let ref_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("#:")).collect();
        assert_eq!(ref_lines, vec!["#: a.py:1 b.py:2"]);
    }

    #[test]
    fn test_wrap_references_breaks_at_80() {
        // Build refs so adding the next ref would make the line >= 80.
        // "#: " (3) + "aaaa.py:1" (9) repeated. After 7 refs separated by spaces:
        // 2 + (1+9)*7 = 72; adding an 8th: 72 + 1 + 9 = 82 >= 80 -> wrap.
        let refs: Vec<&str> = vec![
            "aaaa.py:1",
            "aaaa.py:2",
            "aaaa.py:3",
            "aaaa.py:4",
            "aaaa.py:5",
            "aaaa.py:6",
            "aaaa.py:7",
            "aaaa.py:8",
        ];
        let entry = make_entry(refs);
        let out = format_entry(&entry, LocationMode::Full, false, false, false);
        let ref_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("#:")).collect();
        assert_eq!(ref_lines.len(), 2);
        for l in &ref_lines {
            // For this input (9-char refs), all wrapped lines fit in < 80 chars.
            // Note: a single ref ≥ 77 chars on a fresh "#:" line CAN produce a
            // line of exactly 80 chars; that is covered by test_wrap_references_exact_80_wraps.
            assert!(l.len() < 80, "line too long: {} ({})", l, l.len());
        }
    }

    #[test]
    fn test_wrap_references_exact_80_wraps() {
        // Construct a ref that puts the line exactly at 80 chars; xgettext/Django
        // wrap when the result would be >= 80, so this must split.
        // "#:" (2) + " " (1) + ref => 80 means ref length = 77.
        let long_ref = format!("a/{}.py:1", "x".repeat(70));
        assert_eq!(long_ref.len(), 77);
        let entry = make_entry(vec!["a.py:1", &long_ref]);
        let out = format_entry(&entry, LocationMode::Full, false, false, false);
        let ref_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("#:")).collect();
        assert_eq!(ref_lines.len(), 2);
        assert_eq!(ref_lines[0], "#: a.py:1");
        assert_eq!(ref_lines[1], format!("#: {long_ref}"));
    }

    #[test]
    fn test_no_wrap_keeps_single_line() {
        let refs: Vec<&str> = vec![
            "aaaa.py:1",
            "aaaa.py:2",
            "aaaa.py:3",
            "aaaa.py:4",
            "aaaa.py:5",
            "aaaa.py:6",
            "aaaa.py:7",
            "aaaa.py:8",
        ];
        let entry = make_entry(refs);
        let out = format_entry(&entry, LocationMode::Full, false, false, true);
        let ref_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("#:")).collect();
        assert_eq!(ref_lines.len(), 1);
        assert!(ref_lines[0].len() > 80);
    }

    #[test]
    fn test_location_mode_file_strips_line_numbers_and_dedups() {
        let entry = make_entry(vec!["a.py:1", "a.py:2", "b.py:3"]);
        let out = format_entry(&entry, LocationMode::File, false, false, true);
        let ref_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("#:")).collect();
        assert_eq!(ref_lines, vec!["#: a.py b.py"]);
    }

    #[test]
    fn test_location_mode_never_omits_location_line() {
        let entry = make_entry(vec!["a.py:1"]);
        let out = format_entry(&entry, LocationMode::Never, false, false, true);
        assert!(!out.lines().any(|l| l.starts_with("#:")));
    }

    fn opts(no_obsolete: bool) -> PoFileOptions {
        PoFileOptions {
            location_mode: LocationMode::Never,
            no_obsolete,
            no_wrap: true,
            sort_output: true,
            no_fuzzy_matching: true,
            no_flags: false,
            keep_header: true,
        }
    }

    fn entry(msgid: &str) -> TranslationEntry {
        TranslationEntry {
            msgid: msgid.to_string(),
            msgid_plural: None,
            msgctxt: None,
            references: vec!["a.py:1".to_string()],
            comments: Vec::new(),
        }
    }

    const EXISTING: &str = r#"msgid ""
msgstr ""
"Language: en\n"

msgid "Kept"
msgstr "translated kept"

msgid "Gone"
msgstr "precious translation"
"#;

    #[test]
    fn test_obsolete_entry_is_preserved() {
        let out = merge_entries(&[entry("Kept")], Some(EXISTING), "en", &opts(false));
        assert!(
            out.contains(r#"#~ msgid "Gone""#),
            "missing obsolete block:\n{out}"
        );
        assert!(out.contains(r#"#~ msgstr "precious translation""#));
        assert!(out.contains(r#"msgid "Kept""#));
    }

    #[test]
    fn test_no_obsolete_drops_entry() {
        let out = merge_entries(&[entry("Kept")], Some(EXISTING), "en", &opts(true));
        assert!(
            !out.contains("Gone"),
            "obsolete entry should be gone:\n{out}"
        );
    }

    #[test]
    fn test_untranslated_obsolete_is_not_kept() {
        let existing = "msgid \"\"\nmsgstr \"\"\n\nmsgid \"Gone\"\nmsgstr \"\"\n";
        let out = merge_entries(&[entry("Kept")], Some(existing), "en", &opts(false));
        assert!(!out.contains("Gone"));
    }

    #[test]
    fn test_obsolete_survives_round_trip() {
        let first = merge_entries(&[entry("Kept")], Some(EXISTING), "en", &opts(false));
        let second = merge_entries(&[entry("Kept")], Some(&first), "en", &opts(false));
        assert_eq!(second.matches(r#"#~ msgid "Gone""#).count(), 1, "{second}");
    }

    #[test]
    fn test_obsolete_entry_has_no_location_line() {
        let out = merge_entries(&[entry("Kept")], Some(EXISTING), "en", &{
            let mut o = opts(false);
            o.location_mode = LocationMode::Full;
            o
        });
        assert!(!out.contains("#~ #:"));
    }

    #[test]
    fn test_translator_comment_emitted() {
        let mut e = entry("Hi");
        e.comments = vec!["Translators: a hint".to_string()];
        let out = merge_entries(&[e], None, "en", &opts(false));
        assert!(out.contains("#. Translators: a hint"), "{out}");
    }
}
