mod extractor;
mod plural_forms;
mod po;
mod walker;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use po::LocationMode;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[value(rename_all = "lower")]
enum AddLocation {
    Full,
    File,
    Never,
}

#[derive(Parser, Debug)]
#[command(
    name = "django-makemessages-rs",
    about = "A fast Rust replacement for Django's makemessages command"
)]
struct Cli {
    /// Locales to generate (e.g. -l en -l zh_Hant)
    #[arg(short = 'l', long = "locale")]
    locales: Vec<String>,

    /// Locales to exclude (repeatable)
    #[arg(short = 'x', long = "exclude")]
    exclude: Vec<String>,

    /// Update the message files for all existing locales
    #[arg(short = 'a', long)]
    all: bool,

    /// Follow symlinks to directories when scanning
    #[arg(short = 's', long)]
    symlinks: bool,

    /// Patterns to ignore (directories/files)
    #[arg(short = 'i', long = "ignore")]
    ignore_patterns: Vec<String>,

    /// Don't ignore the default patterns: CVS, .*, *~, *.pyc
    #[arg(long)]
    no_default_ignore: bool,

    /// Don't write '#: filename:line' lines (shorthand for --add-location never)
    #[arg(long, conflicts_with = "add_location")]
    no_location: bool,

    /// Controls '#:' location comments: full (default), file, or never
    #[arg(long, value_enum)]
    add_location: Option<AddLocation>,

    /// Remove obsolete message strings
    #[arg(long)]
    no_obsolete: bool,

    /// Don't break long message lines into several lines
    #[arg(long)]
    no_wrap: bool,

    /// Generate sorted output
    #[arg(long)]
    sort_output: bool,

    /// Do not use fuzzy matching
    #[arg(long)]
    no_fuzzy_matching: bool,

    /// Don't write '#, flags' lines
    #[arg(long)]
    no_flags: bool,

    /// Keep the header of the .po file
    #[arg(long)]
    keep_header: bool,

    /// Exit with error if .po files would be modified (dry-run check)
    #[arg(long)]
    check: bool,

    /// Root directory to scan (default: current directory)
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Locale directory path (default: ./locale)
    #[arg(long, default_value = "locale")]
    locale_dir: PathBuf,

    /// Additional locale directories, like Django's LOCALE_PATHS (repeatable)
    #[arg(long = "locale-path")]
    locale_paths: Vec<PathBuf>,

    /// Write messages into each app's own locale/ dir, like Django does
    #[arg(long)]
    per_app_locale: bool,

    /// Domain name (default: django)
    #[arg(short = 'd', long, default_value = "django")]
    domain: String,

    /// File extensions to examine [default: html,txt,py; js for djangojs]
    #[arg(short = 'e', long = "extension")]
    extensions: Vec<String>,

    /// Show timing information
    #[arg(long)]
    timing: bool,
}

/// Django's is_valid_locale: `^[a-z]+$` or `^[a-z]+_[A-Z0-9].*$`.
fn is_valid_locale(locale: &str) -> bool {
    let mut parts = locale.splitn(2, '_');
    let lang = parts.next().unwrap_or("");
    if lang.is_empty() || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    match parts.next() {
        None => true,
        Some(rest) => rest
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
    }
}

/// Mirrors Django's suggestion for a mistyped locale, e.g. `en-gb` -> `en_GB`.
fn suggest_locale(locale: &str) -> Option<String> {
    let split = locale.find(|c: char| !c.is_ascii_alphabetic())?;
    let (lang, rest) = locale.split_at(split);
    let territory = &rest[1..];
    if lang.is_empty() || territory.is_empty() {
        return None;
    }
    let head: String = territory
        .chars()
        .take(2)
        .flat_map(|c| c.to_uppercase())
        .collect();
    let tail: String = territory.chars().skip(2).collect();
    Some(format!("{}_{head}{tail}", lang.to_lowercase()))
}

/// Locales present under a locale dir, matching Django's `[a-z]{2}` filter.
fn existing_locales(locale_dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(locale_dir) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let mut cs = name.chars();
            let ok = matches!((cs.next(), cs.next()), (Some(a), Some(b))
                if a.is_ascii_lowercase() && b.is_ascii_lowercase());
            if ok {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

fn resolve_locales(cli: &Cli, locale_dir: &std::path::Path) -> Result<Vec<String>> {
    let all_locales = existing_locales(locale_dir);
    // Django ignores --exclude when --all is given.
    let selected: Vec<String> = if cli.all {
        all_locales
    } else {
        let base = if cli.locales.is_empty() {
            all_locales
        } else {
            cli.locales.clone()
        };
        base.into_iter()
            .filter(|l| !cli.exclude.contains(l))
            .collect()
    };

    let mut valid = Vec::new();
    for locale in selected {
        if is_valid_locale(&locale) {
            valid.push(locale);
            continue;
        }
        match suggest_locale(&locale).filter(|s| is_valid_locale(s)) {
            Some(s) => eprintln!("invalid locale {locale}, did you mean {s}?"),
            None => eprintln!("invalid locale {locale}"),
        }
    }
    Ok(valid)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let start = Instant::now();

    let root = std::fs::canonicalize(&cli.root).context("Failed to resolve root directory")?;
    let locale_dir = if cli.locale_dir.is_absolute() {
        cli.locale_dir.clone()
    } else {
        root.join(&cli.locale_dir)
    };

    // Django requires at least one of --locale / --exclude / --all.
    if cli.locales.is_empty() && cli.exclude.is_empty() && !cli.all {
        anyhow::bail!("specify at least one of --locale, --exclude or --all");
    }
    if cli.domain != "django" && cli.domain != "djangojs" {
        anyhow::bail!("currently only the 'django' and 'djangojs' domains are supported");
    }

    let locales = resolve_locales(&cli, &locale_dir)?;
    if locales.is_empty() {
        anyhow::bail!("no locales to process");
    }

    eprintln!("Scanning files in {}...", root.display());
    let file_start = Instant::now();

    // Django defaults extensions by domain: js for djangojs, html/txt/py else.
    let extensions = if !cli.extensions.is_empty() {
        cli.extensions.clone()
    } else if cli.domain == "djangojs" {
        vec!["js".to_string()]
    } else {
        vec!["html".to_string(), "txt".to_string(), "py".to_string()]
    };

    // --locale-dir stays first so it remains the default target; extra
    // --locale-path entries mirror Django's LOCALE_PATHS.
    let mut locale_paths = vec![locale_dir.clone()];
    for p in &cli.locale_paths {
        let abs = if p.is_absolute() {
            p.clone()
        } else {
            root.join(p)
        };
        if !locale_paths.contains(&abs) {
            locale_paths.push(abs);
        }
    }

    // Django's default ignore patterns, unless --no-default-ignore.
    let mut ignore_patterns = cli.ignore_patterns.clone();
    if !cli.no_default_ignore {
        for p in ["CVS", ".*", "*~", "*.pyc"] {
            let p = p.to_string();
            if !ignore_patterns.contains(&p) {
                ignore_patterns.push(p);
            }
        }
    }

    let file_walker = walker::FileWalker::new(
        root.clone(),
        extensions.clone(),
        ignore_patterns,
        locale_paths,
        cli.per_app_locale,
        cli.symlinks,
    );
    let files = file_walker.walk()?;
    let file_count = files.len();

    if cli.timing {
        eprintln!("  Found {} files in {:?}", file_count, file_start.elapsed());
    }

    eprintln!("Extracting translation strings...");
    let extract_start = Instant::now();

    let extracted: Vec<(PathBuf, Vec<extractor::TranslationEntry>)> = files
        .par_iter()
        .filter_map(|file| {
            let rel_path = file.path.strip_prefix(&root).unwrap_or(&file.path);
            match extractor::extract_file(&file.path) {
                Ok(mut entries) => {
                    for entry in &mut entries {
                        entry.references = entry
                            .references
                            .iter()
                            .map(|r| {
                                r.replace(
                                    &file.path.to_string_lossy().to_string(),
                                    &rel_path.to_string_lossy().to_string(),
                                )
                            })
                            .collect();
                    }
                    Some((file.locale_dir.clone(), entries))
                }
                Err(e) => {
                    eprintln!(
                        "Warning: failed to extract from {}: {}",
                        file.path.display(),
                        e
                    );
                    None
                }
            }
        })
        .collect();

    // Group by the locale dir each file was assigned to; without
    // --per-app-locale this is a single group.
    let mut by_locale_dir: BTreeMap<PathBuf, Vec<extractor::TranslationEntry>> = BTreeMap::new();
    for (dir, entries) in extracted {
        by_locale_dir.entry(dir).or_default().extend(entries);
    }

    let total_strings: usize = by_locale_dir.values().map(|v| v.len()).sum();
    if cli.timing {
        eprintln!(
            "  Extracted {} strings in {:?}",
            total_strings,
            extract_start.elapsed()
        );
    }

    eprintln!("Generating PO files for {} locale(s)...", locales.len());
    let po_start = Instant::now();

    let location_mode = if cli.no_location {
        LocationMode::Never
    } else {
        match cli.add_location {
            Some(AddLocation::Full) | None => LocationMode::Full,
            Some(AddLocation::File) => LocationMode::File,
            Some(AddLocation::Never) => LocationMode::Never,
        }
    };

    let options = po::PoFileOptions {
        location_mode,
        no_obsolete: cli.no_obsolete,
        no_wrap: cli.no_wrap,
        sort_output: cli.sort_output,
        no_fuzzy_matching: cli.no_fuzzy_matching,
        no_flags: cli.no_flags,
        keep_header: cli.keep_header,
    };

    let mut changed_files = Vec::new();

    for (dir, entries) in &by_locale_dir {
        for locale in &locales {
            let po_path = dir
                .join(locale)
                .join("LC_MESSAGES")
                .join(format!("{}.po", cli.domain));

            let existing_content = if po_path.exists() {
                Some(std::fs::read_to_string(&po_path).context("Failed to read existing PO file")?)
            } else {
                None
            };

            let merged = po::merge_entries(entries, existing_content.as_deref(), locale, &options);

            let new_content = format!("{merged}\n");

            if cli.check {
                let old = existing_content.unwrap_or_default();
                if old != new_content {
                    changed_files.push(po_path.display().to_string());
                }
            } else {
                po::write_po_file(&po_path, &merged)?;
                eprintln!("  Wrote {}", po_path.display());
            }
        }
    }

    if cli.timing {
        eprintln!("  PO generation took {:?}", po_start.elapsed());
    }

    let elapsed = start.elapsed();
    eprintln!(
        "Done: {} files scanned, {} strings extracted, {} locale(s) {} in {:.2}s",
        file_count,
        total_strings,
        locales.len(),
        if cli.check { "checked" } else { "updated" },
        elapsed.as_secs_f64()
    );

    if cli.check && !changed_files.is_empty() {
        eprintln!("The following .po files are out of sync:");
        for f in &changed_files {
            eprintln!("  {f}");
        }
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_locales() {
        for l in ["en", "fr", "en_GB", "zh_Hant", "sr_Latn", "nl_NL", "es_419"] {
            assert!(is_valid_locale(l), "{l} should be valid");
        }
    }

    #[test]
    fn test_invalid_locales() {
        for l in ["en-gb", "EN", "en_gb", "", "En"] {
            assert!(!is_valid_locale(l), "{l} should be invalid");
        }
    }

    #[test]
    fn test_suggests_canonical_form() {
        assert_eq!(suggest_locale("en-gb").as_deref(), Some("en_GB"));
        assert_eq!(suggest_locale("pt-br").as_deref(), Some("pt_BR"));
    }

    #[test]
    fn test_suggestion_keeps_tail_after_two_chars() {
        // Django uppercases only the first two territory chars.
        assert_eq!(
            suggest_locale("nl-nl-x-informal").as_deref(),
            Some("nl_NL-x-informal")
        );
    }
}
