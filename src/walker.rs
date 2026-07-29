use anyhow::Result;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// A source file paired with the locale directory its messages belong to.
pub struct TranslatableFile {
    pub path: PathBuf,
    pub locale_dir: PathBuf,
}

pub struct FileWalker {
    root: PathBuf,
    extensions: Vec<String>,
    ignore_patterns: Vec<String>,
    /// Locale dirs given up front (`--locale-dir`, and Django's LOCALE_PATHS).
    locale_paths: Vec<PathBuf>,
    /// Discover `locale/` dirs during the walk, the way Django does.
    discover_locale_dirs: bool,
    follow_symlinks: bool,
}

impl FileWalker {
    pub fn new(
        root: PathBuf,
        extensions: Vec<String>,
        ignore_patterns: Vec<String>,
        locale_paths: Vec<PathBuf>,
        discover_locale_dirs: bool,
        follow_symlinks: bool,
    ) -> Self {
        Self {
            root,
            extensions,
            ignore_patterns,
            locale_paths,
            discover_locale_dirs,
            follow_symlinks,
        }
    }

    pub fn walk(&self) -> Result<Vec<TranslatableFile>> {
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(true)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(false)
            .follow_links(self.follow_symlinks);

        let mut overrides = OverrideBuilder::new(&self.root);

        for pattern in &self.ignore_patterns {
            overrides.add(&format!("!{pattern}/"))?;
            overrides.add(&format!("!{pattern}"))?;
        }

        for ext in &self.extensions {
            overrides.add(&format!("*.{ext}"))?;
        }

        builder.overrides(overrides.build()?);

        let mut files = Vec::new();
        let mut discovered: Vec<PathBuf> = Vec::new();

        for entry in builder.build() {
            let entry = entry?;
            let path = entry.path();

            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                if self.discover_locale_dirs && path.file_name().is_some_and(|n| n == "locale") {
                    discovered.push(path.to_path_buf());
                }
                continue;
            }
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            // Django prunes `locale/` dirs from the walk entirely.
            if self.discover_locale_dirs && path.components().any(|c| c.as_os_str() == "locale") {
                continue;
            }
            files.push(path.to_path_buf());
        }

        files.sort();

        // Django inserts each discovered dir at the front as it walks, so the
        // nearest enclosing `locale/` wins. Ordering deepest-first gives the
        // same result without depending on traversal order.
        discovered.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        let mut search: Vec<PathBuf> = discovered;
        search.extend(self.locale_paths.iter().cloned());

        let default_dir = self
            .locale_paths
            .first()
            .cloned()
            .unwrap_or_else(|| self.root.join("locale"));

        Ok(files
            .into_iter()
            .map(|path| {
                let locale_dir =
                    locale_dir_for(&path, &search).unwrap_or_else(|| default_dir.clone());
                TranslatableFile { path, locale_dir }
            })
            .collect())
    }
}

/// Django assigns a file to the first locale dir whose *parent* is a prefix of
/// the file's directory, i.e. the `locale/` belonging to the app it lives in.
fn locale_dir_for(file: &Path, search: &[PathBuf]) -> Option<PathBuf> {
    let dir = file.parent()?;
    search
        .iter()
        .find(|candidate| candidate.parent().is_some_and(|p| dir.starts_with(p)))
        .cloned()
}
