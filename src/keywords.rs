//! xgettext keyword specifications.
//!
//! A keywordspec is `name[:argnum[c][,argnum[c]...]]`, e.g. `_`,
//! `ngettext:1,2`, `pgettext:1c,2`, `npgettext:1c,2,3`. The `c` suffix marks
//! the argument that carries the message context. Django passes exactly these
//! shapes to xgettext, and django-extended-makemessages' `--detect-aliases`
//! derives the same argnums for import aliases.

/// Which argument positions of a call hold the msgid / plural / context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    pub name: String,
    /// 1-based argument holding the singular msgid.
    pub msgid: usize,
    /// 1-based argument holding the plural msgid, if any.
    pub plural: Option<usize>,
    /// 1-based argument holding the msgctxt, if any.
    pub context: Option<usize>,
}

impl Keyword {
    pub fn simple(name: &str) -> Self {
        Self {
            name: name.to_string(),
            msgid: 1,
            plural: None,
            context: None,
        }
    }

    /// Parse one `--keyword` value. Returns None when the spec is malformed.
    pub fn parse(spec: &str) -> Option<Self> {
        let (name, args) = match spec.split_once(':') {
            None => return Some(Self::simple(spec)),
            Some((n, a)) => (n, a),
        };
        if name.is_empty() {
            return None;
        }

        let mut positions = Vec::new();
        let mut context = None;
        for part in args.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            // A trailing "c" marks the context argument; xgettext also allows
            // a "t" total-count suffix, which carries no message and is ignored.
            if let Some(num) = part.strip_suffix('c') {
                context = Some(num.parse().ok()?);
            } else if let Some(num) = part.strip_suffix('t') {
                let _: usize = num.parse().ok()?;
            } else {
                positions.push(part.parse().ok()?);
            }
        }

        let mut it = positions.into_iter();
        let msgid = it.next()?;
        Some(Self {
            name: name.to_string(),
            msgid,
            plural: it.next(),
            context,
        })
    }
}

/// The gettext functions Django exposes, with the argnums xgettext is told to
/// use. Mirrors django-extended-makemessages' `get_argnums`.
pub fn argnums_for(function: &str) -> Option<&'static str> {
    match function {
        "gettext" | "gettext_lazy" | "gettext_noop" => Some("1"),
        "ngettext" | "ngettext_lazy" => Some("1,2"),
        "npgettext" | "npgettext_lazy" => Some("1c,2,3"),
        "pgettext" | "pgettext_lazy" => Some("1c,2"),
        _ => None,
    }
}

/// Django's own defaults: the keywords it passes to xgettext plus xgettext's
/// built-ins (`gettext`, `_`, `ngettext:1,2`), which Django relies on.
pub fn default_keywords(domain_is_js: bool) -> Vec<Keyword> {
    let mut specs = vec![
        "gettext",
        "_",
        "gettext_noop",
        "gettext_lazy",
        "ngettext:1,2",
        "ngettext_lazy:1,2",
        "pgettext:1c,2",
        "npgettext:1c,2,3",
    ];
    // Django omits the _lazy context variants for the djangojs domain.
    if !domain_is_js {
        specs.push("pgettext_lazy:1c,2");
        specs.push("npgettext_lazy:1c,2,3");
    }
    specs.iter().filter_map(|s| Keyword::parse(s)).collect()
}

/// `from django.utils.translation import gettext as _, ngettext as ng`
/// Only direct imports from that module count, matching --detect-aliases.
static IMPORT_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(
        r"(?s)from\s+django\.utils\.translation\s+import\s+(\(.*?\)|[^\n]*(?:\\\n[^\n]*)*)",
    )
    .unwrap()
});

/// Extract `name as alias` pairs for known gettext functions, yielding the
/// keywordspec each alias should be scanned with.
pub fn detect_aliases(content: &str) -> Vec<Keyword> {
    let mut out = Vec::new();
    for caps in IMPORT_RE.captures_iter(content) {
        let list = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let list = list.trim().trim_start_matches('(').trim_end_matches(')');
        for item in list.split(',') {
            let item = item.replace('\\', " ");
            let mut parts = item.split_whitespace();
            let (Some(name), Some(kw), Some(alias)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if kw != "as" {
                continue;
            }
            let Some(argnums) = argnums_for(name) else {
                continue;
            };
            if let Some(k) = Keyword::parse(&format!("{alias}:{argnums}")) {
                out.push(k);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_simple_alias() {
        let ks = detect_aliases("from django.utils.translation import gettext as _\n");
        assert_eq!(ks.len(), 1);
        assert_eq!(ks[0].name, "_");
        assert_eq!(ks[0].msgid, 1);
    }

    #[test]
    fn test_detect_alias_keeps_argnums() {
        let ks = detect_aliases("from django.utils.translation import pgettext_lazy as pl\n");
        assert_eq!(ks[0].name, "pl");
        assert_eq!(ks[0].context, Some(1));
        assert_eq!(ks[0].msgid, 2);
    }

    #[test]
    fn test_detect_multiple_and_parenthesized() {
        let src =
            "from django.utils.translation import (\n    gettext as g,\n    ngettext as ng,\n)\n";
        let names: Vec<String> = detect_aliases(src).iter().map(|k| k.name.clone()).collect();
        assert_eq!(names, vec!["g", "ng"]);
    }

    #[test]
    fn test_plain_import_is_not_an_alias() {
        assert!(detect_aliases("from django.utils.translation import gettext\n").is_empty());
    }

    #[test]
    fn test_other_module_ignored() {
        assert!(detect_aliases("from mymod import gettext as _\n").is_empty());
    }

    #[test]
    fn test_bare_name() {
        let k = Keyword::parse("_").unwrap();
        assert_eq!(k.name, "_");
        assert_eq!(k.msgid, 1);
        assert_eq!(k.plural, None);
        assert_eq!(k.context, None);
    }

    #[test]
    fn test_plural_spec() {
        let k = Keyword::parse("ngettext:1,2").unwrap();
        assert_eq!(k.msgid, 1);
        assert_eq!(k.plural, Some(2));
        assert_eq!(k.context, None);
    }

    #[test]
    fn test_context_spec() {
        let k = Keyword::parse("pgettext:1c,2").unwrap();
        assert_eq!(k.context, Some(1));
        assert_eq!(k.msgid, 2);
        assert_eq!(k.plural, None);
    }

    #[test]
    fn test_context_plural_spec() {
        let k = Keyword::parse("npgettext:1c,2,3").unwrap();
        assert_eq!(k.context, Some(1));
        assert_eq!(k.msgid, 2);
        assert_eq!(k.plural, Some(3));
    }

    #[test]
    fn test_non_first_msgid() {
        let k = Keyword::parse("mylog:2").unwrap();
        assert_eq!(k.msgid, 2);
    }

    #[test]
    fn test_total_count_suffix_ignored() {
        let k = Keyword::parse("dngettext:2,3,4t").unwrap();
        assert_eq!(k.msgid, 2);
        assert_eq!(k.plural, Some(3));
    }

    #[test]
    fn test_malformed_specs_rejected() {
        assert!(Keyword::parse(":1").is_none());
        assert!(Keyword::parse("name:x").is_none());
        assert!(Keyword::parse("name:1c").is_none());
    }

    #[test]
    fn test_argnums_match_django() {
        assert_eq!(argnums_for("gettext_lazy"), Some("1"));
        assert_eq!(argnums_for("ngettext_lazy"), Some("1,2"));
        assert_eq!(argnums_for("pgettext_lazy"), Some("1c,2"));
        assert_eq!(argnums_for("npgettext_lazy"), Some("1c,2,3"));
        assert_eq!(argnums_for("not_a_gettext_fn"), None);
    }

    #[test]
    fn test_djangojs_omits_lazy_context_variants() {
        let js: Vec<String> = default_keywords(true)
            .iter()
            .map(|k| k.name.clone())
            .collect();
        assert!(!js.contains(&"pgettext_lazy".to_string()));
        let py: Vec<String> = default_keywords(false)
            .iter()
            .map(|k| k.name.clone())
            .collect();
        assert!(py.contains(&"pgettext_lazy".to_string()));
    }
}
