"""Normalize a .po file to a comparable form: drop the header entry and all
location lines, keep msgctxt/msgid/msgid_plural/#. comments/obsolete markers.

Emitted in file order so ordering differences show up too.
"""

import re
import sys


def unquote(parts):
    out = []
    for p in parts:
        m = re.match(r'^\s*"(.*)"\s*$', p)
        if m:
            out.append(m.group(1))
    return "".join(out)


def entries(path):
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().splitlines()

    cur = {"comments": [], "obsolete": False}
    field = None
    buf = []

    def flush():
        if field:
            cur[field] = unquote(buf)

    for line in lines:
        stripped = line.strip()
        if stripped.startswith("#~"):
            cur["obsolete"] = True
            stripped = stripped[2:].strip()
        if stripped.startswith("#."):
            cur["comments"].append(stripped)
            continue
        if stripped.startswith("#:") or stripped.startswith("#|"):
            continue
        if stripped.startswith("#,"):
            cur["comments"].append(stripped)
            continue
        if stripped.startswith("#"):
            continue
        m = re.match(r"^(msgctxt|msgid_plural|msgid|msgstr(?:\[\d+\])?)\s+(.*)$", stripped)
        if m:
            flush()
            field = m.group(1)
            buf = [m.group(2)]
            continue
        if stripped.startswith('"') and field:
            buf.append(stripped)
            continue
        if not stripped:
            flush()
            if "msgid" in cur:
                yield cur
            cur = {"comments": [], "obsolete": False}
            field = None
            buf = []
    flush()
    if "msgid" in cur:
        yield cur


def main():
    for e in entries(sys.argv[1]):
        # Skip the PO header entry (empty msgid, no context).
        if e.get("msgid") == "" and not e.get("msgctxt"):
            continue
        for c in e["comments"]:
            if c.startswith("#."):
                print(c)
        tag = "OBSOLETE " if e["obsolete"] else ""
        if e.get("msgctxt") is not None:
            print(f"{tag}msgctxt {e['msgctxt']!r}")
        print(f"{tag}msgid {e.get('msgid', '')!r}")
        if e.get("msgid_plural") is not None:
            print(f"{tag}msgid_plural {e['msgid_plural']!r}")
        print()


if __name__ == "__main__":
    main()
