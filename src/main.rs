mod extractor;
mod po;
mod walker;

use anyhow::{Context, Result};
use clap::Parser;
use rayon::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "django-makemessages-rs",
    about = "A fast Rust replacement for Django's makemessages command"
)]
struct Cli {
    /// Locales to generate (e.g. -l en -l zh_Hant)
    #[arg(short = 'l', long = "locale", required = true)]
    locales: Vec<String>,

    /// Patterns to ignore (directories/files)
    #[arg(short = 'i', long = "ignore")]
    ignore_patterns: Vec<String>,

    /// Don't write '#: filename:line' lines
    #[arg(long)]
    no_location: bool,

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

    /// Root directory to scan (default: current directory)
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Locale directory path (default: ./locale)
    #[arg(long, default_value = "locale")]
    locale_dir: PathBuf,

    /// Domain name (default: django)
    #[arg(short = 'd', long, default_value = "django")]
    domain: String,

    /// File extensions to examine
    #[arg(short = 'e', long = "extension", default_values_t = vec!["html".to_string(), "txt".to_string(), "py".to_string()])]
    extensions: Vec<String>,

    /// Show timing information
    #[arg(long)]
    timing: bool,
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

    eprintln!("Scanning files in {}...", root.display());
    let file_start = Instant::now();

    let file_walker = walker::FileWalker::new(
        root.clone(),
        cli.extensions.clone(),
        cli.ignore_patterns.clone(),
    );
    let files = file_walker.walk()?;
    let file_count = files.len();

    if cli.timing {
        eprintln!(
            "  Found {} files in {:?}",
            file_count,
            file_start.elapsed()
        );
    }

    eprintln!("Extracting translation strings...");
    let extract_start = Instant::now();

    let all_entries: Vec<extractor::TranslationEntry> = files
        .par_iter()
        .filter_map(|file| {
            let rel_path = file.strip_prefix(&root).unwrap_or(file);
            match extractor::extract_file(file) {
                Ok(mut entries) => {
                    for entry in &mut entries {
                        entry.references = entry
                            .references
                            .iter()
                            .map(|r| {
                                r.replace(&file.to_string_lossy().to_string(), &rel_path.to_string_lossy().to_string())
                            })
                            .collect();
                    }
                    Some(entries)
                }
                Err(e) => {
                    eprintln!("Warning: failed to extract from {}: {}", file.display(), e);
                    None
                }
            }
        })
        .flatten()
        .collect();

    let total_strings = all_entries.len();
    if cli.timing {
        eprintln!(
            "  Extracted {} strings in {:?}",
            total_strings,
            extract_start.elapsed()
        );
    }

    eprintln!("Generating PO files for {} locale(s)...", cli.locales.len());
    let po_start = Instant::now();

    let options = po::PoFileOptions {
        no_location: cli.no_location,
        no_obsolete: cli.no_obsolete,
        no_wrap: cli.no_wrap,
        sort_output: cli.sort_output,
        no_fuzzy_matching: cli.no_fuzzy_matching,
        no_flags: cli.no_flags,
        keep_header: cli.keep_header,
    };

    for locale in &cli.locales {
        let po_path = locale_dir
            .join(locale)
            .join("LC_MESSAGES")
            .join(format!("{}.po", cli.domain));

        let existing_content = if po_path.exists() {
            Some(std::fs::read_to_string(&po_path).context("Failed to read existing PO file")?)
        } else {
            None
        };

        let merged = po::merge_entries(
            &all_entries,
            existing_content.as_deref(),
            locale,
            &options,
        );

        po::write_po_file(&po_path, &merged)?;
        eprintln!("  Wrote {}", po_path.display());
    }

    if cli.timing {
        eprintln!("  PO generation took {:?}", po_start.elapsed());
    }

    let elapsed = start.elapsed();
    eprintln!(
        "Done: {} files scanned, {} strings extracted, {} locale(s) updated in {:.2}s",
        file_count,
        total_strings,
        cli.locales.len(),
        elapsed.as_secs_f64()
    );

    Ok(())
}
