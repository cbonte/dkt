#!/usr/bin/env python3
"""Writes the NOTICE of the project from the crates that the binary links.

usage: scripts/notice.py          write NOTICE
       scripts/notice.py --check  exit 1 when NOTICE is not what the crates give

A crate is linked when the binary reaches it through normal dependencies. The crates that only run
at build time, the procedural macros and what only they use, are left out.
"""
import re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-unknown-linux-gnu"

HEADER = """dkt
Copyright (c) 2026 Cyril Bonté

Third-party software linked into the dkt binary. Each crate keeps its own licence.

"""
FOOTER = """
The complete list, with versions, comes from Cargo.lock.

syntect embeds the syntax definitions and colour themes it ships with its default features.
They remain under the terms published in the syntect repository.
"""

def tree():
    out = subprocess.run(["cargo", "tree", "-e", "normal", "--prefix", "depth", "--no-dedupe",
                          "--target", TARGET, "--format", "{p} {l}"],
                         cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return out.splitlines()

def linked(lines):
    """The crates reached through normal dependencies, without the subtrees of procedural macros."""
    crates = {}
    skip_below = None
    for line in lines:
        m = re.match(r"(\d+)(\S+) v(\S+)((?: \([^)]*\))*) ?(.*)$", line)
        if not m:
            continue
        depth, name, markers, lic = int(m.group(1)), m.group(2), m.group(4), m.group(5)
        if skip_below is not None and depth > skip_below:
            continue
        skip_below = None
        if depth == 0:
            continue
        if "proc-macro" in markers:
            skip_below = depth
            continue
        crates[name] = lic
    return crates

def licence(text):
    text = (text or "unknown").replace("/", " OR ")
    parts = [p.strip() for p in text.replace(" OR ", "|").split("|") if p.strip()]
    return " or ".join(dict.fromkeys(parts))

def render(crates):
    rows = sorted((name, licence(lic)) for name, lic in crates.items())
    left = max(len(n) for n, _ in rows + [("Crate", "")])
    right = max(len(k) for _, k in rows + [("", "Licence")])
    table = [f"| {'Crate'.ljust(left)} | {'Licence'.ljust(right)} |", f"| {'-' * left} | {'-' * right} |"]
    table += [f"| {n.ljust(left)} | {k.ljust(right)} |" for n, k in rows]
    return HEADER + "\n".join(table) + "\n" + FOOTER

if __name__ == "__main__":
    text = render(linked(tree()))
    path = ROOT / "NOTICE"
    if "--check" in sys.argv:
        if path.read_text() != text:
            print("NOTICE is not what the crates of the binary give: run scripts/notice.py")
            sys.exit(1)
    else:
        path.write_text(text)
