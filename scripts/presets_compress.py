#!/usr/bin/env python3
"""Aggressive leaf-level compressor for platform-presets.json.

Folds the following leaves onto single lines (key + value on one line), while
keeping structural layers (top-level / protocol objects / section keys) and
peak_hours windows multi-line for editability:

- ``model_list``  protocol field       -> entire value single line
- ``name``        protocol field       -> entire value single line (8 locale map)
- ``desc``        protocol field       -> entire value single line (8 locale map)
- ``source_urls`` protocol field       -> entire value single line
- ``models``      protocol field       -> wrapper multi-line, each branch slot
                                           map (e.g. ``default``) single line
- ``endpoints``   protocol field       -> wrapper multi-line; each branch
                                           (``default`` / ``coding_plan``) is a
                                           list whose endpoint objects render
                                           one per line, inline

Field order is preserved (Python dicts keep insertion order; ``json.load``
preserves source order). JSON semantics are untouched: only whitespace /
newlines change. Round-trip equivalence (``json.load(orig) == json.load(new)``)
is the hard gate (see R3.1 in the task PRD).

Usage::

    python3 scripts/presets_compress.py [INPUT] [OUTPUT]

Defaults: INPUT  = src-tauri/defaults/platform-presets.json
          OUTPUT = (overwrites INPUT if omitted)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# protocol-level keys whose entire value collapses onto a single line.
INLINE_VALUE_KEYS = {"model_list", "name", "desc", "source_urls"}

# protocol-level keys rendered as multi-line wrappers with inline children.
# ``models`` keeps its wrapper multi-line (one line per branch) with each
# branch's slot map inline; ``endpoints`` renders endpoint objects one per
# line. ``peak_hours`` is intentionally untouched (default multi-line
# recursion) so its window objects stay editable.
WRAPPER_KEYS = {"models", "endpoints"}


def inline(obj) -> str:
    """Render any JSON value on a single line using standard separators."""
    return json.dumps(obj, ensure_ascii=False, separators=(", ", ": "))


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
            if key in INLINE_VALUE_KEYS:
                # entire value onto one line
                lines.append(f"{inner_pad}{jk}: {inline(value)}{comma}")
            elif key == "models" and isinstance(value, dict):
                # wrapper multi-line; each branch (default/coding_plan) inline
                branch_lines: list[str] = []
                bitems = list(value.items())
                for bi, (bk, bv) in enumerate(bitems):
                    bcomma = "" if bi == len(bitems) - 1 else ","
                    bjk = json.dumps(bk, ensure_ascii=False)
                    branch_lines.append(
                        f"{'  ' * (indent + 2)}{bjk}: {inline(bv)}{bcomma}"
                    )
                lines.append(f"{inner_pad}{jk}: {{")
                lines.extend(branch_lines)
                lines.append(f"{inner_pad}}}{comma}")
            elif key == "endpoints" and isinstance(value, dict):
                # wrapper multi-line; each branch is a list of endpoint objects,
                # one inline object per line
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
                # default: standard multi-line recursion (peak_hours, scalars,
                # any unexpected shape)
                lines.append(
                    f"{inner_pad}{jk}: {serialize(value, indent + 1)}{comma}"
                )
        return "{\n" + "\n".join(lines) + f"\n{pad}}}"
    if isinstance(obj, list):
        if not obj:
            return "[]"
        inner_pad = "  " * (indent + 1)
        # default list: each element on its own line (peak_hours windows, etc.)
        items = list(obj)
        lines = []
        for idx, item in enumerate(items):
            last = idx == len(items) - 1
            comma = "" if last else ","
            lines.append(f"{inner_pad}{serialize(item, indent + 1)}{comma}")
        return "[\n" + "\n".join(lines) + f"\n{pad}]"
    # scalar
    return json.dumps(obj, ensure_ascii=False)


def main(argv: list[str]) -> int:
    repo = Path(__file__).resolve().parent.parent
    default_path = repo / "src-tauri" / "defaults" / "platform-presets.json"
    in_path = Path(argv[1]) if len(argv) > 1 else default_path
    out_path = Path(argv[2]) if len(argv) > 2 else in_path

    data = json.loads(in_path.read_text(encoding="utf-8"))
    rendered = serialize(data, 0)
    if not out_path.parent.exists():
        out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(rendered + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
