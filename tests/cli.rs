//! End-to-end tests for CLI behavior that unit tests cannot reach: exit
//! codes, which files get created, and flags whose whole effect is in main.rs.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn binary() -> PathBuf {
    // tests run from the crate root; the harness lives next to the binary.
    let mut p = std::env::current_exe().expect("test exe path");
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("django-makemessages-rs")
}

struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("dmr-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn write(&self, rel: &str, contents: &str) -> &Self {
        let path = self.dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        self
    }

    fn mkdir(&self, rel: &str) -> &Self {
        fs::create_dir_all(self.dir.join(rel)).unwrap();
        self
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(binary())
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("failed to run binary")
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.path(rel)).unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Fill in every empty `msgstr ""` except the header's, so a catalog reads as
/// fully translated.
fn translate_all(po: &str) -> String {
    let mut out = String::new();
    let mut seen_header = false;
    for line in po.lines() {
        if line == "msgstr \"\"" {
            if seen_header {
                out.push_str("msgstr \"TRANSLATED\"\n");
                continue;
            }
            seen_header = true;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

const PO_EN: &str = "locale/en/LC_MESSAGES/django.po";

fn source_with(msgids: &[&str]) -> String {
    let mut s = String::from("from django.utils.translation import gettext as _\n");
    for (i, m) in msgids.iter().enumerate() {
        s.push_str(&format!("x{i} = _(\"{m}\")\n"));
    }
    s
}

#[test]
fn requires_a_locale_selector() {
    let p = Project::new("no-selector");
    p.write("v.py", &source_with(&["hi"]));
    let out = p.run(&["--locale-dir", "locale"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("at least one of"), "{err}");
}

#[test]
fn rejects_unknown_domain() {
    let p = Project::new("bad-domain");
    p.write("v.py", &source_with(&["hi"]));
    let out = p.run(&["-l", "en", "-d", "nope", "--locale-dir", "locale"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("domain"));
}

#[test]
fn check_exits_nonzero_when_out_of_sync() {
    let p = Project::new("check-dirty");
    p.write("v.py", &source_with(&["fresh string"]));
    let out = p.run(&["-l", "en", "--locale-dir", "locale", "--check"]);
    assert!(!out.status.success(), "--check should fail for a new file");
    assert!(!p.exists(PO_EN), "--check must not write");
}

#[test]
fn check_exits_zero_when_in_sync() {
    let p = Project::new("check-clean");
    p.write("v.py", &source_with(&["stable"]));
    assert!(p
        .run(&["-l", "en", "--locale-dir", "locale"])
        .status
        .success());
    let out = p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--check",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn dry_run_leaves_tree_untouched() {
    let p = Project::new("dry-run");
    p.write("v.py", &source_with(&["one"]));
    p.run(&["-l", "en", "--locale-dir", "locale"]);
    let before = p.read(PO_EN);

    p.write("v.py", &source_with(&["one", "two"]));
    let out = p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--dry-run",
    ]);
    assert!(out.status.success());
    assert_eq!(p.read(PO_EN), before, "--dry-run must not write");
}

#[test]
fn no_untranslated_fails_then_passes() {
    let p = Project::new("untranslated");
    p.write("v.py", &source_with(&["needs work"]));
    p.run(&["-l", "en", "--locale-dir", "locale"]);

    let out = p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--no-untranslated",
    ]);
    assert!(!out.status.success(), "empty msgstr must fail");
    assert!(String::from_utf8_lossy(&out.stderr).contains("needs work"));

    let translated = translate_all(&p.read(PO_EN));
    fs::write(p.path(PO_EN), translated).unwrap();
    let out = p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--no-untranslated",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn no_untranslated_ignores_the_header() {
    // The header's own empty msgid/msgstr must never count as untranslated.
    let p = Project::new("untranslated-header");
    p.write("v.py", &source_with(&["a"]));
    p.run(&["-l", "en", "--locale-dir", "locale"]);
    let translated = translate_all(&p.read(PO_EN));
    fs::write(p.path(PO_EN), translated).unwrap();
    assert!(p
        .run(&[
            "-l",
            "en",
            "--locale-dir",
            "locale",
            "--keep-header",
            "--no-untranslated"
        ])
        .status
        .success());
}

#[test]
fn compile_produces_a_readable_mo() {
    if which("msgfmt").is_none() {
        eprintln!("skipping: msgfmt not installed");
        return;
    }
    let p = Project::new("compile");
    p.write("v.py", &source_with(&["compile me"]));
    p.run(&["-l", "en", "--locale-dir", "locale"]);
    // msgfmt drops untranslated entries, so translate before compiling.
    let translated = translate_all(&p.read(PO_EN));
    fs::write(p.path(PO_EN), translated).unwrap();

    let out = p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--compile",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mo = p.path("locale/en/LC_MESSAGES/django.mo");
    assert!(mo.exists(), "--compile must produce a .mo");
    let dump = Command::new("msgunfmt").arg(&mo).output().unwrap();
    assert!(dump.status.success(), "msgfmt produced an unreadable .mo");
    assert!(String::from_utf8_lossy(&dump.stdout).contains("compile me"));
}

#[test]
fn compile_is_skipped_for_check_and_dry_run() {
    if which("msgfmt").is_none() {
        return;
    }
    let p = Project::new("compile-skip");
    p.write("v.py", &source_with(&["x"]));
    p.run(&["-l", "en", "--locale-dir", "locale", "--check", "--compile"]);
    assert!(!p.exists("locale/en/LC_MESSAGES/django.mo"));
}

#[test]
fn force_po_writes_an_empty_catalog() {
    let p = Project::new("force-po");
    p.write("v.py", "x = 1  # nothing translatable\n");

    p.run(&["-l", "en", "--locale-dir", "locale"]);
    assert!(!p.exists(PO_EN), "empty catalog should be skipped");

    assert!(p
        .run(&["-l", "en", "--locale-dir", "locale", "--force-po"])
        .status
        .success());
    assert!(p.exists(PO_EN), "--force-po must write the catalog");
}

#[test]
fn all_processes_every_existing_locale() {
    let p = Project::new("all");
    p.write("v.py", &source_with(&["s"]));
    for l in ["en", "fr", "de"] {
        p.mkdir(&format!("locale/{l}/LC_MESSAGES"));
    }
    assert!(p.run(&["--all", "--locale-dir", "locale"]).status.success());
    for l in ["en", "fr", "de"] {
        assert!(
            p.exists(&format!("locale/{l}/LC_MESSAGES/django.po")),
            "{l} missing"
        );
    }
}

#[test]
fn exclude_skips_a_locale() {
    let p = Project::new("exclude");
    p.write("v.py", &source_with(&["s"]));
    for l in ["en", "fr", "de"] {
        p.mkdir(&format!("locale/{l}/LC_MESSAGES"));
    }
    assert!(p
        .run(&["-x", "fr", "--locale-dir", "locale"])
        .status
        .success());
    assert!(p.exists("locale/en/LC_MESSAGES/django.po"));
    assert!(p.exists("locale/de/LC_MESSAGES/django.po"));
    assert!(
        !p.exists("locale/fr/LC_MESSAGES/django.po"),
        "fr was excluded"
    );
}

#[test]
fn all_ignores_exclude_like_django() {
    // Django's own quirk: --all wins and --exclude is dropped.
    let p = Project::new("all-exclude");
    p.write("v.py", &source_with(&["s"]));
    for l in ["en", "fr"] {
        p.mkdir(&format!("locale/{l}/LC_MESSAGES"));
    }
    assert!(p
        .run(&["--all", "-x", "fr", "--locale-dir", "locale"])
        .status
        .success());
    assert!(
        p.exists("locale/fr/LC_MESSAGES/django.po"),
        "--all must ignore --exclude"
    );
}

#[test]
fn invalid_locale_is_reported_and_skipped() {
    let p = Project::new("invalid-locale");
    p.write("v.py", &source_with(&["s"]));
    p.mkdir("locale/en/LC_MESSAGES");
    let out = p.run(&["-l", "en-gb", "-l", "en", "--locale-dir", "locale"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("invalid locale en-gb, did you mean en_GB?"),
        "{err}"
    );
    assert!(
        p.exists(PO_EN),
        "the valid locale should still be processed"
    );
}

#[test]
fn default_ignore_patterns_skip_dotdirs() {
    let p = Project::new("default-ignore");
    p.write("v.py", &source_with(&["visible"]));
    p.write(".hidden/v.py", &source_with(&["hidden away"]));
    p.run(&["-l", "en", "--locale-dir", "locale"]);
    let po = p.read(PO_EN);
    assert!(po.contains("visible"));
    assert!(
        !po.contains("hidden away"),
        "dotdirs are ignored by default"
    );
}

#[test]
fn ignore_pattern_excludes_a_directory() {
    let p = Project::new("ignore");
    p.write("v.py", &source_with(&["kept"]));
    p.write("vendor/v.py", &source_with(&["dropped"]));
    p.run(&["-l", "en", "--locale-dir", "locale", "-i", "vendor"]);
    let po = p.read(PO_EN);
    assert!(po.contains("kept"));
    assert!(!po.contains("dropped"));
}

#[test]
fn per_app_locale_splits_by_app() {
    let p = Project::new("per-app");
    p.write("appA/models.py", &source_with(&["from app A"]));
    p.write("core/models.py", &source_with(&["from core"]));
    p.mkdir("appA/locale");
    p.mkdir("locale");
    assert!(p
        .run(&["-l", "en", "--locale-dir", "locale", "--per-app-locale"])
        .status
        .success());

    let app = p.read("appA/locale/en/LC_MESSAGES/django.po");
    assert!(app.contains("from app A"));
    assert!(
        !app.contains("from core"),
        "app catalog must not absorb core"
    );

    let root = p.read(PO_EN);
    assert!(root.contains("from core"));
    assert!(!root.contains("from app A"));
}

#[test]
fn default_layout_keeps_one_catalog() {
    let p = Project::new("flat");
    p.write("appA/models.py", &source_with(&["from app A"]));
    p.write("core/models.py", &source_with(&["from core"]));
    p.mkdir("appA/locale");
    p.run(&["-l", "en", "--locale-dir", "locale"]);
    let root = p.read(PO_EN);
    assert!(root.contains("from app A") && root.contains("from core"));
    assert!(!p.exists("appA/locale/en/LC_MESSAGES/django.po"));
}

#[test]
fn djangojs_domain_extracts_js_and_names_the_file() {
    let p = Project::new("djangojs");
    p.write("app.js", "const a = gettext(\"js string\");\n");
    assert!(p
        .run(&["-l", "en", "-d", "djangojs", "--locale-dir", "locale"])
        .status
        .success());
    let po = p.read("locale/en/LC_MESSAGES/djangojs.po");
    assert!(po.contains("js string"), "{po}");
}

#[test]
fn obsolete_entries_survive_by_default() {
    let p = Project::new("obsolete");
    p.write("v.py", &source_with(&["kept"]));
    p.run(&["-l", "en", "--locale-dir", "locale"]);
    let translated = translate_all(&p.read(PO_EN));
    fs::write(p.path(PO_EN), translated).unwrap();

    // Remove the string from the source; its translation must be retained.
    p.write("v.py", "x = 1\n");
    p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--force-po",
    ]);
    let po = p.read(PO_EN);
    assert!(po.contains("#~ msgid \"kept\""), "{po}");

    p.run(&[
        "-l",
        "en",
        "--locale-dir",
        "locale",
        "--keep-header",
        "--force-po",
        "--no-obsolete",
    ]);
    assert!(
        !p.read(PO_EN).contains("kept"),
        "--no-obsolete should drop it"
    );
}

#[test]
fn width_zero_disables_wrapping() {
    let p = Project::new("width");
    let mut src = String::from("from django.utils.translation import gettext as _\n");
    for i in 0..12 {
        src.push_str(&format!("x{i} = _(\"shared\")\n"));
    }
    p.write("a.py", &src);
    p.run(&["-l", "en", "--locale-dir", "locale", "--width", "0"]);
    let refs = p
        .read(PO_EN)
        .lines()
        .filter(|l| l.starts_with("#:"))
        .count();
    assert_eq!(refs, 1, "--width 0 must emit a single location line");
}

fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(program))
        .find(|p: &PathBuf| p.is_file())
}
