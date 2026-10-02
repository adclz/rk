#!/usr/bin/env python3
"""SPDX headers on the Rust sources under the AGPL.

LICENSING.md puts the project under AGPL-3.0-only, with directories under
Apache-2.0 or MIT that carry their own LICENSE. Every Rust source under the
AGPL starts with two SPDX lines: `--fix` adds them where they are missing,
and `--check`, which CI runs, refuses a file without them. The generated
AST gets its lines from crates/ast/build.rs.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

HEADER = [
    "// SPDX-FileCopyrightText: 2026 Clauzel Adrien",
    "// SPDX-License-Identifier: AGPL-3.0-only",
]

# LICENSING.md: the directories under another license.
OTHER_LICENSES = (
    "crates/debug_format/",
    "crates/index/",
    "crates/macros/",
    "crates/tree-sitter/",
    "crates/wasm_builtins/",
    "crates/wasm_builtins_generated/",
    "stdlib/",
    "vscode/syntaxes/",
)


def agpl_rust_sources() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "-z", "--", "*.rs"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [p for p in out.split("\0") if p and not p.startswith(OTHER_LICENSES)]


def has_header(text: str) -> bool:
    return text.splitlines()[: len(HEADER)] == HEADER


def check() -> int:
    missing = [p for p in agpl_rust_sources() if not has_header((ROOT / p).read_text())]
    for path in missing:
        print(f"{path}: no SPDX header (python3 scripts/spdx.py --fix)")
    if missing:
        print(f"{len(missing)} Rust source(s) under the AGPL without the SPDX lines")
        return 1
    print("every Rust source under the AGPL starts with its SPDX lines")
    return 0


def fix() -> int:
    added = 0
    for path in agpl_rust_sources():
        file = ROOT / path
        text = file.read_text()
        if has_header(text):
            continue
        eol = "\r\n" if "\r\n" in text[:200] else "\n"
        file.write_text(eol.join(HEADER) + eol + eol + text, newline="")
        added += 1
    print(f"added {added} header(s)")
    return 0


def main(argv: list[str]) -> int:
    if argv == ["--check"]:
        return check()
    if argv == ["--fix"]:
        return fix()
    print(__doc__.strip(), file=sys.stderr)
    print("\nusage: scripts/spdx.py --check | --fix", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
