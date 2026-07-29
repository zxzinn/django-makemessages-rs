#!/usr/bin/env bash
#
# Differential test: run the real Django `makemessages` and this tool over the
# same fixture tree, then compare the extracted messages.
#
# Headers are excluded from the comparison: xgettext writes its own boilerplate
# and a live POT-Creation-Date, neither of which this tool tries to reproduce.
# Locations are excluded too (they are covered by unit tests).
#
# Requires GNU gettext and a Python with Django available. Set up with:
#   python3 -m venv .venv && .venv/bin/pip install django
#
# Usage: tests/differential/run.sh [fixture-name ...]

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
FIXTURES="$HERE/fixtures"

PYTHON="${PYTHON:-$HERE/.venv/bin/python}"
RS_BIN="${RS_BIN:-$ROOT/target/release/django-makemessages-rs}"

if [ ! -x "$RS_BIN" ]; then
    echo "error: $RS_BIN not found; run: cargo build --release" >&2
    exit 2
fi
if ! "$PYTHON" -c "import django" 2>/dev/null; then
    echo "error: Django not importable via $PYTHON" >&2
    echo "hint:  python3 -m venv $HERE/.venv && $HERE/.venv/bin/pip install django" >&2
    exit 2
fi
for prog in xgettext msgmerge msguniq msgattrib; do
    command -v "$prog" >/dev/null || { echo "error: $prog not found (install gettext)" >&2; exit 2; }
done

names=("$@")
if [ ${#names[@]} -eq 0 ]; then
    names=()
    for d in "$FIXTURES"/*/; do names+=("$(basename "$d")"); done
fi

pass=0
fail=0

for name in "${names[@]}"; do
    fixture="$FIXTURES/$name"
    [ -d "$fixture" ] || { echo "SKIP   $name (no such fixture)"; continue; }

    # The djangojs domain is selected by fixture name.
    domain=django
    [ "$name" = "jsdomain" ] && domain=djangojs

    work=$(mktemp -d)

    mkdir -p "$work/dj" "$work/rs"
    cp -R "$fixture"/. "$work/dj"/
    cp -R "$fixture"/. "$work/rs"/
    cp "$HERE/settings.py" "$HERE/manage.py" "$work/dj"/
    mkdir -p "$work/dj/locale" "$work/rs/locale"

    (cd "$work/dj" && DJANGO_SETTINGS_MODULE=settings "$PYTHON" manage.py makemessages \
        -d "$domain" -l en --no-obsolete -i settings.py -i manage.py) >/dev/null 2>&1
    (cd "$work/rs" && "$RS_BIN" -d "$domain" -l en --locale-dir locale) >/dev/null 2>&1

    dj_po="$work/dj/locale/en/LC_MESSAGES/$domain.po"
    rs_po="$work/rs/locale/en/LC_MESSAGES/$domain.po"

    "$PYTHON" "$HERE/normalize.py" "$dj_po" > "$work/dj.txt" 2>/dev/null
    "$PYTHON" "$HERE/normalize.py" "$rs_po" > "$work/rs.txt" 2>/dev/null

    if diff -u --label django --label rust "$work/dj.txt" "$work/rs.txt" > "$work/diff.txt"; then
        echo "ok     $name"
        pass=$((pass + 1))
    else
        echo "FAIL   $name"
        sed -n '3,$p' "$work/diff.txt" | sed 's/^/       /'
        fail=$((fail + 1))
    fi
    rm -rf "$work"
done

echo
echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
