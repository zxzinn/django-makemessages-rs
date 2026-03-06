#!/bin/bash
set -e

DJANGO_PROJECT="${DJANGO_PROJECT:?Set DJANGO_PROJECT to your Django project root}"
RUST_BIN="${RUST_BIN:-$(dirname "$0")/../target/release/django-makemessages-rs}"
TMPDIR=$(mktemp -d)

trap 'rm -rf "$TMPDIR"' EXIT

echo "=== Integration Test: Django vs Rust makemessages ==="
echo ""

cd "$DJANGO_PROJECT"

echo "[1/4] Running Django extendedmakemessages..."
cp -r locale "$TMPDIR/locale_backup"

export DATABASE_URL="${DATABASE_URL:-sqlite:///tmp/dummy.db}"
export CELERY_BROKER_URL="${CELERY_BROKER_URL:-memory://}"
export REDIS_URL="${REDIS_URL:-redis://localhost:6379/0}"
export USE_DOCKER="${USE_DOCKER:-no}"

uv run python manage.py extendedmakemessages \
  -l en -l zh_Hant -l zh_Hans -l ko -l ja \
  --verbosity=0 \
  --ignore=.venv --ignore=venv --ignore=test --ignore=tests \
  --ignore=node_modules --ignore=staticfiles --ignore=media \
  --ignore=dist --ignore=docs --ignore=exports --ignore=documents \
  --ignore=logs --ignore=compose --ignore=k8s --ignore=shell_scripts \
  --ignore=automatic-fixture-export --ignore=test_media \
  --ignore="*.pyc" --ignore="__pycache__" \
  --no-location --no-obsolete --sort-output \
  --no-fuzzy-matching --no-flags --no-wrap --keep-header 2>/dev/null

for lang in en zh_Hant zh_Hans ko ja; do
  cp "locale/$lang/LC_MESSAGES/django.po" "$TMPDIR/django_${lang}.po"
done

cp -r "$TMPDIR/locale_backup/"* locale/

echo "[2/4] Running Rust django-makemessages-rs..."

"$RUST_BIN" \
  -l en -l zh_Hant -l zh_Hans -l ko -l ja \
  --ignore .venv --ignore venv --ignore test --ignore tests \
  --ignore node_modules --ignore staticfiles --ignore media \
  --ignore dist --ignore docs --ignore exports --ignore documents \
  --ignore logs --ignore compose --ignore k8s --ignore shell_scripts \
  --ignore automatic-fixture-export --ignore test_media \
  --no-location --no-flags --sort-output --no-fuzzy-matching \
  --keep-header \
  --locale-dir locale 2>/dev/null

for lang in en zh_Hant zh_Hans ko ja; do
  cp "locale/$lang/LC_MESSAGES/django.po" "$TMPDIR/rust_${lang}.po"
done

cp -r "$TMPDIR/locale_backup/"* locale/

echo "[3/4] Comparing outputs..."
echo ""

PASS=true

for lang in en zh_Hant zh_Hans ko ja; do
  DJANGO_FILE="$TMPDIR/django_${lang}.po"
  RUST_FILE="$TMPDIR/rust_${lang}.po"

  DJANGO_COUNT=$(grep -c '^msgid ' "$DJANGO_FILE" || true)
  RUST_COUNT=$(grep -c '^msgid ' "$RUST_FILE" || true)

  DJANGO_MSGIDS=$(grep '^msgid ' "$DJANGO_FILE" | sort)
  RUST_MSGIDS=$(grep '^msgid ' "$RUST_FILE" | sort)

  MISSING=$(diff <(echo "$DJANGO_MSGIDS") <(echo "$RUST_MSGIDS") | grep '^< ' | wc -l | tr -d ' ')
  EXTRA=$(diff <(echo "$DJANGO_MSGIDS") <(echo "$RUST_MSGIDS") | grep '^> ' | wc -l | tr -d ' ')

  DJANGO_TRANSLATED=$(grep -c '^msgstr ".' "$DJANGO_FILE" || true)
  RUST_TRANSLATED=$(grep -c '^msgstr ".' "$RUST_FILE" || true)

  if [ "$DJANGO_COUNT" = "$RUST_COUNT" ] && [ "$MISSING" = "0" ] && [ "$EXTRA" = "0" ]; then
    STATUS="PASS"
  else
    STATUS="FAIL"
    PASS=false
  fi

  echo "  [$STATUS] $lang: Django=$DJANGO_COUNT Rust=$RUST_COUNT missing=$MISSING extra=$EXTRA translated:Django=$DJANGO_TRANSLATED Rust=$RUST_TRANSLATED"

  if [ "$MISSING" != "0" ]; then
    echo "    Missing in Rust:"
    diff <(echo "$DJANGO_MSGIDS") <(echo "$RUST_MSGIDS") | grep '^< ' | head -10 | sed 's/^/      /'
  fi
  if [ "$EXTRA" != "0" ]; then
    echo "    Extra in Rust:"
    diff <(echo "$DJANGO_MSGIDS") <(echo "$RUST_MSGIDS") | grep '^> ' | head -10 | sed 's/^/      /'
  fi
done

echo ""

echo "[4/4] Byte-level comparison (excluding POT-Creation-Date)..."
for lang in en zh_Hant zh_Hans ko ja; do
  DJANGO_FILE="$TMPDIR/django_${lang}.po"
  RUST_FILE="$TMPDIR/rust_${lang}.po"

  DJANGO_NORM=$(grep -v "POT-Creation-Date" "$DJANGO_FILE" | grep -v "PO-Revision-Date")
  RUST_NORM=$(grep -v "POT-Creation-Date" "$RUST_FILE" | grep -v "PO-Revision-Date")

  DIFF_LINES=$(diff <(echo "$DJANGO_NORM") <(echo "$RUST_NORM") | grep '^[<>]' | wc -l | tr -d ' ')

  if [ "$DIFF_LINES" = "0" ]; then
    echo "  [PASS] $lang: byte-identical (excluding dates)"
  else
    echo "  [DIFF] $lang: $DIFF_LINES differing lines"
    diff <(echo "$DJANGO_NORM") <(echo "$RUST_NORM") | head -30 | sed 's/^/    /'
    PASS=false
  fi
done

echo ""
if [ "$PASS" = true ]; then
  echo "=== ALL TESTS PASSED ==="
  exit 0
else
  echo "=== SOME TESTS FAILED ==="
  exit 1
fi
