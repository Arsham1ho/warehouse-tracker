#!/usr/bin/env python3
"""AI-assistance review gate.

Blocks a change from reaching the main branch when it touches code that was
written with AI assistance, until a human reviewer has formally signed off and
that sign-off is recorded.

The rules this enforces:

  1. Declare it.   A commit that used AI assistance must say so in its message
                   with a trailer:  AI-Assisted: <tool>
  2. Review it.    Such a commit must also carry:  Reviewed-by: <name>
  3. Record it.    Every approved AI-assisted change is appended to
                   docs/ai-assistance-log.md, so the record outlives the PR.

The point is not to discourage AI use. It is that a reviewer took
responsibility for the output, and that the fact is auditable afterwards
rather than remembered.
"""
from __future__ import annotations

import pathlib
import re
import subprocess
import sys

LOG = pathlib.Path("docs/ai-assistance-log.md")
AI_TRAILER = re.compile(r"^AI-Assisted:\s*(.+)$", re.M)
REVIEW_TRAILER = re.compile(r"^Reviewed-by:\s*(.+)$", re.M)


def commits_in_range(base: str, head: str) -> list[str]:
    out = subprocess.run(
        ["git", "rev-list", f"{base}..{head}"],
        capture_output=True, text=True, check=True,
    ).stdout
    return [c for c in out.split() if c]


def message(sha: str) -> str:
    return subprocess.run(
        ["git", "log", "-1", "--format=%B", sha],
        capture_output=True, text=True, check=True,
    ).stdout


def files(sha: str) -> list[str]:
    out = subprocess.run(
        ["git", "show", "--name-only", "--format=", sha],
        capture_output=True, text=True, check=True,
    ).stdout
    return [f for f in out.splitlines() if f.strip()]


def logged(sha: str) -> bool:
    if not LOG.exists():
        return False
    return sha[:12] in LOG.read_text(encoding="utf-8")


def main(base: str, head: str) -> int:
    failures: list[str] = []

    for sha in commits_in_range(base, head):
        msg = message(sha)
        ai = AI_TRAILER.search(msg)
        if not ai:
            continue  # not declared as AI-assisted; nothing to gate

        short, subject = sha[:12], msg.splitlines()[0]

        reviewer = REVIEW_TRAILER.search(msg)
        if not reviewer:
            failures.append(
                f"{short} {subject}\n"
                f"    declared AI-Assisted: {ai.group(1).strip()}\n"
                f"    but carries no 'Reviewed-by:' trailer. A human must review "
                f"AI-assisted code before it reaches a deliverable."
            )
            continue

        if not logged(sha):
            failures.append(
                f"{short} {subject}\n"
                f"    reviewed by {reviewer.group(1).strip()} but not recorded in {LOG}.\n"
                f"    Append an entry so the approval is auditable."
            )
            continue

        print(f"ok  {short}  AI-assisted, reviewed by {reviewer.group(1).strip()}, recorded")
        for f in files(sha):
            print(f"        touched: {f}")

    if failures:
        print("\nAI review gate failed:\n", file=sys.stderr)
        for f in failures:
            print(f"  - {f}\n", file=sys.stderr)
        return 1

    print("AI review gate passed.")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("usage: ai_review_gate.py <base-ref> <head-ref>", file=sys.stderr)
        raise SystemExit(2)
    raise SystemExit(main(sys.argv[1], sys.argv[2]))
