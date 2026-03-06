# django-makemessages-rs

A fast Rust replacement for Django's `makemessages` command. Produces byte-identical `.po` file output compared to [django-extended-makemessages](https://pypi.org/project/django-extended-makemessages/), but roughly 60-70x faster.

Tested against a ~2000 file Django project with ~3000 translatable strings across 5 locales:

- Django `extendedmakemessages`: ~21s
- `django-makemessages-rs`: ~0.3s

## Install

```
pip install django-makemessages-rs
```

Platform wheels are available for macOS (arm64, x86_64) and Linux (x86_64, aarch64).

## Usage

```
django-makemessages-rs \
  -l en -l zh_Hant -l zh_Hans -l ko -l ja \
  --ignore .venv --ignore node_modules \
  --no-location --no-flags --sort-output \
  --no-fuzzy-matching --keep-header \
  --locale-dir locale
```

### Options

```
-l, --locale <LOCALES>       Locales to generate (required, repeatable)
-i, --ignore <PATTERNS>      Patterns to ignore (directories/files)
-d, --domain <DOMAIN>        Domain name [default: django]
-e, --extension <EXTS>       File extensions to examine [default: html txt py]
    --root <PATH>            Root directory to scan [default: .]
    --locale-dir <PATH>      Locale directory [default: locale]
    --no-location            Don't write #: filename:line lines
    --no-flags               Don't write #, flags lines
    --sort-output            Generate sorted output
    --no-fuzzy-matching      Do not use fuzzy matching
    --keep-header            Keep the existing .po file header
    --no-obsolete            Remove obsolete message strings
    --no-wrap                Don't break long message lines
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
- `{% blocktrans trimmed %}` whitespace collapsing
- `{% blocktrans %}...{% plural %}...{% endblocktrans %}` plural forms
- Python implicit string concatenation (`_("foo" "bar")`)
- Template variable substitution (`{{ var }}` to `%(var)s`)
- Literal `%` escaping to `%%`

## Pre-commit integration

Add to your `pyproject.toml` dev dependencies:

```toml
"django-makemessages-rs==0.1.4"
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

## License

MIT
