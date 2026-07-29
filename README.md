# django-makemessages-rs

[![PyPI](https://img.shields.io/pypi/v/django-makemessages-rs)](https://pypi.org/project/django-makemessages-rs/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://opensource.org/licenses/MIT)

A fast Rust alternative to Django's `makemessages` command. Produces byte-identical `.po` file output compared to [django-extended-makemessages](https://pypi.org/project/django-extended-makemessages/).

Tested against a ~2000 file Django project with ~3000 translatable strings across 5 locales:

| Tool | Time |
|------|------|
| `django-extended-makemessages` | ~21s |
| `django-makemessages-rs` | ~0.3s |

## Install

```
pip install django-makemessages-rs
```

Platform wheels are available for macOS (arm64, x86_64) and Linux (x86_64, aarch64).

## Usage

```bash
django-makemessages-rs \
  -l en -l zh_Hant -l zh_Hans -l ko -l ja \
  --ignore .venv --ignore node_modules \
  --no-location --no-flags --sort-output \
  --no-fuzzy-matching --keep-header \
  --locale-dir locale
```

### CI check mode

Use `--check` to verify translation files are in sync with source code. Exits with code 1 if any `.po` file would change:

```bash
django-makemessages-rs \
  -l en -l zh_Hant \
  --ignore .venv --ignore node_modules \
  --no-location --no-flags --sort-output \
  --no-fuzzy-matching --keep-header \
  --locale-dir locale \
  --check
```

### Options

```
-l, --locale <LOCALES>       Locales to generate (repeatable)
-x, --exclude <LOCALES>      Locales to exclude (repeatable)
-a, --all                    Update all existing locales
-s, --symlinks               Follow symlinks to directories when scanning
-i, --ignore <PATTERNS>      Patterns to ignore (directories/files)
    --no-default-ignore      Don't ignore CVS, .*, *~, *.pyc
-d, --domain <DOMAIN>        Domain name: django or djangojs [default: django]
-e, --extension <EXTS>       File extensions to examine [default: html txt py, or js for djangojs]
-k, --keyword <SPEC>         Extra xgettext keywordspec, e.g. -k t or -k t:1c,2
                             (repeatable; -k '' drops the defaults)
    --add-comments [TAG]     Emit preceding comments as #. lines
                             [default tag: Translators; bare = all comments]
    --detect-aliases         Treat `import gettext as x` aliases as keywords
    --root <PATH>            Root directory to scan [default: .]
    --locale-dir <PATH>      Locale directory [default: locale]
    --locale-path <PATH>     Extra locale directories, like LOCALE_PATHS (repeatable)
    --per-app-locale         Write into each app's own locale/ dir, like Django
    --no-location            Don't write #: filename:line lines (shorthand for --add-location never)
    --add-location <MODE>    Controls #: location comments: full (default), file, or never
    --no-flags               Don't write #, flags lines
    --sort-output            Generate sorted output
    --no-fuzzy-matching      Do not use fuzzy matching
    --keep-header            Keep the existing .po file header
    --no-obsolete            Remove obsolete message strings
    --no-wrap                Don't break long message lines
    --check                  Exit with error if .po files would change (dry-run)
    --timing                 Show timing information
```

## How it works

1. Walks the project tree using [ignore](https://crates.io/crates/ignore) (same engine as ripgrep)
2. Extracts translatable strings from `.py` and `.html`/`.txt` templates in parallel using [rayon](https://crates.io/crates/rayon)
3. Merges extracted strings with existing `.po` files, preserving translations
4. Writes updated `.po` files

The extractor handles:
- Python `gettext()`, `ngettext()`, `pgettext()`, `npgettext()` and the `_()` alias
- Django template tags: `{% trans %}`, `{% translate %}`, `{% blocktrans %}`, `{% blocktranslate %}`
- `context "..."` on both `{% translate %}` and `{% blocktranslate %}`, emitted as `msgctxt`
- `{% blocktrans trimmed %}` whitespace collapsing
- `{% blocktrans %}...{% plural %}...{% endblocktrans %}` plural forms
- `_("...")` constants in block tags, variable expressions and filter arguments
  (`{{ foo|default:_("bar") }}`)
- `Translators:` comments, from `#` in Python and `{# ... #}` / `{% comment %}`
  in templates, written out as `#.` lines
- Python implicit string concatenation (`_("foo" "bar")`) and triple-quoted strings
- Template variable substitution, including filters and dotted lookups
  (`{{ user.name|upper }}` to `%(user.name|upper)s`)
- Literal `%` escaping to `%%`
- JavaScript sources under `--domain djangojs`
- custom translation functions via `--keyword`, using xgettext's keywordspec
  syntax (`name`, `name:2`, `name:1c,2`, `name:1,2`)

Only arguments that are *entirely* string literals are extracted, matching
xgettext: `_(getattr(obj, 'verbose_name', label))` yields nothing, while
`_("a" "b")` yields `ab`.

Entries that disappear from the source are kept as `#~` obsolete blocks so
existing translations survive, matching gettext. Pass `--no-obsolete` to drop
them instead.

New `.po` files get the correct `Plural-Forms` for their locale, taken from a
table generated out of Django's own shipped catalogs (98 locales; `ja`, `ko`
and `zh_*` are `nplurals=1`, Russian and Polish get their 4-form rules, and so
on). Unknown locales fall back to the base language, then to
`nplurals=2; plural=(n != 1);`.

### Per-app locale directories

By default everything is written to a single `--locale-dir`. Pass
`--per-app-locale` to follow Django's layout instead: any directory named
`locale/` is treated as a locale root for the app containing it, and each
file's messages go to the nearest enclosing one.

```
appA/locale/en/LC_MESSAGES/django.po   <- strings from appA/
appB/locale/en/LC_MESSAGES/django.po   <- strings from appB/
locale/en/LC_MESSAGES/django.po        <- everything else
```

No Django settings or `DJANGO_SETTINGS_MODULE` required — runs as a standalone CLI.

## Pre-commit / Git hooks integration

Add to your `pyproject.toml` dev dependencies:

```toml
"django-makemessages-rs"
```

Then in your pre-commit script:

```bash
uv run django-makemessages-rs \
  -l en -l zh_Hant \
  --ignore .venv --ignore node_modules \
  --no-location --no-flags --sort-output \
  --no-fuzzy-matching --keep-header \
  --locale-dir locale
```

## Testing

```bash
cargo test --release
```

There is also a differential suite that runs the real Django `makemessages`
over the same fixtures and compares the extracted messages, so behavioral
drift from Django gets caught:

```bash
python3 -m venv tests/differential/.venv
tests/differential/.venv/bin/pip install django django-extended-makemessages
cargo build --release
./tests/differential/run.sh
```

It needs GNU gettext (`xgettext`, `msgmerge`, `msguniq`, `msgattrib`) on PATH,
plus `django-extended-makemessages` for the fixtures covering `--keyword`,
`--add-comments` and `--detect-aliases`.
Headers and `#:` locations are excluded from the comparison; everything else
(msgid, msgid_plural, msgctxt, `#.` comments, ordering) must match exactly.

## License

MIT
