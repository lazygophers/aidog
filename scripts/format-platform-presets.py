#!/usr/bin/env python3
"""Canonical formatter for platform-presets.json.

Single source of truth for the bundled presets file format. Runs two transforms:

1. **Drop ``desc``** from every protocol entry (64 protocols x 8 locale strings).
   ``desc`` is unused at runtime (Rust reads presets as ``serde_json::Value``;
   frontend ``getProtocolDesc`` + UI consumers were removed 2026-07-10). Stripping
   here keeps the bundled file lean; remote JSON still carrying ``desc`` is
   ignored silently (no serde contract on the field).
2. **Collapse ``peak_hours`` and ``models`` field values onto single lines.**
   Other protocol-level fields keep their existing style (``endpoints`` multi-line
   wrapper with inline endpoint objects; ``model_list`` / ``name`` / ``source_urls``
   already inline; scalars untouched).

Idempotent: parsing the output back into Python and re-running produces byte-
identical output (field order preserved via ``json.loads`` + ``dict`` insertion
order). JSON semantics are untouched - only whitespace / newlines / the dropped
``desc`` field change.

Usage::

    python3 scripts/format-platform-presets.py [INPUT] [OUTPUT]

Defaults: INPUT  = src-tauri/defaults/platform-presets.json
          OUTPUT = (overwrites INPUT if omitted)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

def inline(obj) -> str:
    """Render any JSON value on a single line using standard separators."""
    return json.dumps(obj, ensure_ascii=False, separators=(", ", ": "))


def strip_desc(doc: dict) -> None:
    """Remove ``desc`` from every protocol entry in place (no-op if absent)."""
    protocols = doc.get("protocols")
    if not isinstance(protocols, dict):
        return
    for entry in protocols.values():
        if isinstance(entry, dict):
            entry.pop("desc", None)


def serialize_protocol_entry(entry: dict, indent: int) -> str:
    """Render a protocol entry: every field inline except ``endpoints``.

    ``endpoints`` keeps its wrapper multi-line (one line per branch); each
    branch is a list of endpoint objects rendered one per line, inline.
    Everything else (``models`` / ``peak_hours`` / ``model_list`` / ``name`` /
    ``source_urls`` / ``keywords`` / scalars) collapses onto a single line.
    """
    pad = "  " * indent
    inner_pad = "  " * (indent + 1)
    lines: list[str] = ["{"]
    items = list(entry.items())
    for idx, (key, value) in enumerate(items):
        last = idx == len(items) - 1
        comma = "" if last else ","
        jk = json.dumps(key, ensure_ascii=False)
        if key == "endpoints" and isinstance(value, dict):
            lines.append(f"{inner_pad}{jk}: {{")
            bitems = list(value.items())
            for bi, (bk, bv) in enumerate(bitems):
                bcomma = "" if bi == len(bitems) - 1 else ","
                bjk = json.dumps(bk, ensure_ascii=False)
                if isinstance(bv, list) and bv and all(
                    isinstance(x, dict) for x in bv
                ):
                    lines.append(f"{'  ' * (indent + 2)}{bjk}: [")
                    eitems = list(bv)
                    for ei, ev in enumerate(eitems):
                        ecomma = "" if ei == len(eitems) - 1 else ","
                        lines.append(
                            f"{'  ' * (indent + 3)}{inline(ev)}{ecomma}"
                        )
                    lines.append(f"{'  ' * (indent + 2)}]{bcomma}")
                else:
                    lines.append(
                        f"{'  ' * (indent + 2)}{bjk}: {inline(bv)}{bcomma}"
                    )
            lines.append(f"{inner_pad}}}{comma}")
        else:
            lines.append(f"{inner_pad}{jk}: {inline(value)}{comma}")
    lines.append(f"{pad}}}")
    return "\n".join(lines)


def serialize(obj, indent: int) -> str:
    """Custom JSON serializer preserving structure while folding leaves."""
    pad = "  " * indent
    if isinstance(obj, dict):
        if not obj:
            return "{}"
        inner_pad = "  " * (indent + 1)
        lines: list[str] = []
        items = list(obj.items())
        for idx, (key, value) in enumerate(items):
            last = idx == len(items) - 1
            comma = "" if last else ","
            jk = json.dumps(key, ensure_ascii=False)
            # protocol entries: special-case serializer (inline all but endpoints)
            if key == "protocols" and isinstance(value, dict):
                lines.append(f"{inner_pad}{jk}: {{")
                pitems = list(value.items())
                for pi, (pk, pv) in enumerate(pitems):
                    pcomma = "" if pi == len(pitems) - 1 else ","
                    pjk = json.dumps(pk, ensure_ascii=False)
                    if isinstance(pv, dict):
                        lines.append(
                            f"{'  ' * (indent + 2)}{pjk}: "
                            + serialize_protocol_entry(pv, indent + 2)
                            + pcomma
                        )
                    else:
                        lines.append(
                            f"{'  ' * (indent + 2)}{pjk}: "
                            + inline(pv)
                            + pcomma
                        )
                lines.append(f"{inner_pad}}}{comma}")
            else:
                lines.append(
                    f"{inner_pad}{jk}: {serialize(value, indent + 1)}{comma}"
                )
        return "{\n" + "\n".join(lines) + f"\n{pad}}}"
    if isinstance(obj, list):
        if not obj:
            return "[]"
        inner_pad = "  " * (indent + 1)
        items = list(obj)
        lines = []
        for idx, item in enumerate(items):
            last = idx == len(items) - 1
            comma = "" if last else ","
            lines.append(f"{inner_pad}{serialize(item, indent + 1)}{comma}")
        return "[\n" + "\n".join(lines) + f"\n{pad}]"
    return json.dumps(obj, ensure_ascii=False)


def main(argv: list[str]) -> int:
    repo = Path(__file__).resolve().parent.parent
    default_path = repo / "src-tauri" / "defaults" / "platform-presets.json"
    in_path = Path(argv[1]) if len(argv) > 1 else default_path
    out_path = Path(argv[2]) if len(argv) > 2 else in_path

    data = json.loads(in_path.read_text(encoding="utf-8"))
    strip_desc(data)
    rendered = serialize(data, 0)
    if not out_path.parent.exists():
        out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(rendered + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
