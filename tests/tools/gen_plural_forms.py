#!/usr/bin/env python3
"""Regenerate src/plural_forms.rs from an installed Django.

Django's `copy_plural_forms` reads the Plural-Forms header out of its own
shipped catalogs (django/conf/locale/<locale>/LC_MESSAGES/django.po) when it
creates a new .po file. This tool is a standalone binary with no Django
dependency, so that table is baked in at build time instead.

Usage:
    python3 tests/tools/gen_plural_forms.py > src/plural_forms.rs
"""

import os
import re
import sys

import django

# Django's own regex, same flags (see makemessages.plural_forms_re).
PLURAL_FORMS_RE = re.compile(
    r'^(?P<value>"Plural-Forms.+?\\n")\s*$', re.MULTILINE | re.DOTALL
)


def collect():
    base = os.path.join(os.path.dirname(django.__file__), "conf", "locale")
    rows = []
    for locale in sorted(os.listdir(base)):
        po = os.path.join(base, locale, "LC_MESSAGES", "django.po")
        if not os.path.exists(po):
            continue
        with open(po, encoding="utf-8") as fh:
            match = PLURAL_FORMS_RE.search(fh.read())
        if not match:
            continue
        # The header may be wrapped across several quoted segments; join them.
        parts = re.findall(r'"((?:[^"\\]|\\.)*)"', match.group("value"))
        value = "".join(parts)
        value = value.removeprefix("Plural-Forms: ").removesuffix("\\n")
        rows.append((locale, value))
    return rows


def main():
    rows = collect()
    esc = lambda s: s.replace("\\", "\\\\").replace('"', '\\"')
    out = sys.stdout
    out.write(
        "//! Plural-Forms headers for each locale, mirroring what Django's\n"
        "//! `copy_plural_forms` copies out of its own shipped catalogs in\n"
        "//! `django/conf/locale/<locale>/LC_MESSAGES/django.po`.\n"
        "//!\n"
        f"//! Generated from Django {django.get_version()}. Regenerate with\n"
        "//! `tests/tools/gen_plural_forms.py` when syncing to a newer Django.\n\n"
        "/// Default used when a locale is not in Django's catalog set; this is\n"
        "/// also what gettext falls back to.\n"
        'pub const DEFAULT_PLURAL_FORMS: &str = "nplurals=2; plural=(n != 1);";\n\n'
        "/// (locale, Plural-Forms value) sorted by locale for binary search.\n"
        f"pub static PLURAL_FORMS: [(&str, &str); {len(rows)}] = [\n"
    )
    for locale, value in rows:
        out.write(f'    ("{locale}", "{esc(value)}"),\n')
    out.write(
        "];\n\n"
        "/// Look up a locale's Plural-Forms, falling back to the base language\n"
        "/// (`pt_XX` -> `pt`) the way gettext catalogs are conventionally organized.\n"
        "pub fn plural_forms_for(locale: &str) -> &'static str {\n"
        "    if let Ok(i) = PLURAL_FORMS.binary_search_by(|(k, _)| (*k).cmp(locale)) {\n"
        "        return PLURAL_FORMS[i].1;\n"
        "    }\n"
        "    if let Some((base, _)) = locale.split_once('_') {\n"
        "        if let Ok(i) = PLURAL_FORMS.binary_search_by(|(k, _)| (*k).cmp(base)) {\n"
        "            return PLURAL_FORMS[i].1;\n"
        "        }\n"
        "    }\n"
        "    DEFAULT_PLURAL_FORMS\n"
        "}\n"
    )


if __name__ == "__main__":
    main()
