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
# Fixtures with a dj_command need django-extended-makemessages; without it the
# reference side would silently produce nothing and every diff would "pass".
if ls "$FIXTURES"/*/dj_command >/dev/null 2>&1 &&
   ! "$PYTHON" -c "import django_extended_makemessages" 2>/dev/null; then
    echo "error: django-extended-makemessages not importable via $PYTHON" >&2
    echo "hint:  $PYTHON -m pip install django-extended-makemessages" >&2
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

    # A fixture may carry an `options` file with extra flags for our binary,
    # and a `domain` file to select the gettext domain.
    domain=django
    [ -f "$fixture/domain" ] && domain=$(cat "$fixture/domain")
    rs_extra=""
    [ -f "$fixture/options" ] && rs_extra=$(cat "$fixture/options")
    # `dj_options` carries the equivalent flags for the reference command, used
    # where a feature exists on both sides but is spelled differently.
    dj_extra=""
    [ -f "$fixture/dj_options" ] && dj_extra=$(cat "$fixture/dj_options")

    work=$(mktemp -d)

    mkdir -p "$work/dj" "$work/rs"
    cp -R "$fixture"/. "$work/dj"/
    cp -R "$fixture"/. "$work/rs"/
    rm -f "$work/dj/options" "$work/dj/domain" "$work/rs/options" "$work/rs/domain"
    cp "$HERE/settings.py" "$HERE/manage.py" "$work/dj"/
    mkdir -p "$work/dj/locale" "$work/rs/locale"

    # Fixtures needing flags Django itself lacks (--keyword, --add-comments)
    # run against django-extended-makemessages instead.
    dj_command=makemessages
    [ -f "$fixture/dj_command" ] && dj_command=$(cat "$fixture/dj_command")

    # shellcheck disable=SC2086
    (cd "$work/dj" && DJANGO_SETTINGS_MODULE=settings "$PYTHON" manage.py "$dj_command" \
        -d "$domain" -l en --no-obsolete -i settings.py -i manage.py $dj_extra) >/dev/null 2>&1
    # shellcheck disable=SC2086
    (cd "$work/rs" && "$RS_BIN" -d "$domain" -l en --locale-dir locale $rs_extra) >/dev/null 2>&1

    # Compare every .po produced, keyed by path relative to the tree root, so
    # per-app locale dirs are covered too.
    collect() {
        (cd "$1" && find . -name '*.po' | sort | while read -r po; do
            echo "### $po"
            "$PYTHON" "$HERE/normalize.py" "$po"
        done)
    }
    collect "$work/dj" > "$work/dj.txt" 2>/dev/null
    collect "$work/rs" > "$work/rs.txt" 2>/dev/null

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
