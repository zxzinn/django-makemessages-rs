use anyhow::Result;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::PathBuf;

pub struct FileWalker {
    root: PathBuf,
    extensions: Vec<String>,
    ignore_patterns: Vec<String>,
}

impl FileWalker {
    pub fn new(root: PathBuf, extensions: Vec<String>, ignore_patterns: Vec<String>) -> Self {
        Self {
            root,
            extensions,
            ignore_patterns,
        }
    }

    pub fn walk(&self) -> Result<Vec<PathBuf>> {
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(true)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(false);

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
        for entry in builder.build() {
            let entry = entry?;
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                files.push(entry.into_path());
            }
        }

        files.sort();
        Ok(files)
    }
}
