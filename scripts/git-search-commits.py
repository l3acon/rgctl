#!/usr/bin/env python3
"""Search commit authors/committers/messages via git-filter-repo --dry-run.

Examples:
  ./scripts/git-search-commits.py claude
  ./scripts/git-search-commits.py noreply@anthropic.com
  ./scripts/git-search-commits.py --summary 'Co-Authored-By: Claude'
  ./scripts/git-search-commits.py --people-only saveetha13@gmail.com
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

MATCH_RE = re.compile(
    r"(?P<line>[0-9a-f]{40}\tauthor=.+\tcommitter=.+)",
)


CALLBACK = r"""
import os
needle = os.environ.get("GFR_SEARCH", "")
case_sensitive = os.environ.get("GFR_CASE", "0") == "1"
people_only = os.environ.get("GFR_PEOPLE_ONLY", "0") == "1"
message_only = os.environ.get("GFR_MESSAGE_ONLY", "0") == "1"

author_name = commit.author_name.decode(errors="replace")
author_email = commit.author_email.decode(errors="replace")
committer_name = commit.committer_name.decode(errors="replace")
committer_email = commit.committer_email.decode(errors="replace")
msg = commit.message.decode(errors="replace")
author = "%s <%s>" % (author_name, author_email)
committer = "%s <%s>" % (committer_name, committer_email)

parts = []
if not message_only:
    parts.extend([author_name, author_email, committer_name, committer_email, author, committer])
if not people_only:
    parts.append(msg)
hay = "\n".join(parts)
probe = needle if case_sensitive else needle.lower()
text = hay if case_sensitive else hay.lower()
if probe and probe in text:
    hits = []
    for line in msg.splitlines():
        check = line if case_sensitive else line.lower()
        if probe in check:
            hits.append(line.strip())
    note = "; ".join(hits[:5]) if hits else "(matched author/committer)"
    print("%s\tauthor=%s\tcommitter=%s\t%s" % (
        commit.original_id.decode(),
        author,
        committer,
        note,
    ))
"""


def find_filter_repo(explicit: str | None) -> str:
    if explicit:
        path = Path(explicit).expanduser().resolve()
        if not path.is_file():
            raise SystemExit(f"git-filter-repo not found: {path}")
        return str(path)
    here = Path(__file__).resolve().parent.parent / "git-filter-repo"
    if here.is_file():
        return str(here)
    found = shutil.which("git-filter-repo")
    if found:
        return found
    raise SystemExit(
        "git-filter-repo not found. Put it at ./git-filter-repo, "
        "install it on PATH, or pass --filter-repo PATH"
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Find commits matching an email or string (git-filter-repo dry-run)."
    )
    p.add_argument("query", help="Email or substring to search for")
    p.add_argument(
        "-C",
        "--repo",
        default=".",
        help="Git repository path (default: current directory)",
    )
    p.add_argument(
        "--filter-repo",
        dest="filter_repo",
        help="Path to git-filter-repo executable",
    )
    p.add_argument(
        "-i",
        "--ignore-case",
        action="store_true",
        default=True,
        help="Case-insensitive match (default)",
    )
    p.add_argument(
        "--case-sensitive",
        action="store_true",
        help="Case-sensitive match",
    )
    p.add_argument(
        "--people-only",
        action="store_true",
        help="Search only author/committer name and email",
    )
    p.add_argument(
        "--message-only",
        action="store_true",
        help="Search only commit messages",
    )
    p.add_argument(
        "--summary",
        action="store_true",
        help="Print unique authors with counts after the match list",
    )
    p.add_argument(
        "--authors-only",
        action="store_true",
        help="Print only author summary (no per-commit lines)",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    if args.people_only and args.message_only:
        raise SystemExit("Use only one of --people-only / --message-only")

    repo = Path(args.repo).expanduser().resolve()
    if not (repo / ".git").exists() and not (repo / "HEAD").exists():
        raise SystemExit(f"Not a git repository: {repo}")

    filter_repo = find_filter_repo(args.filter_repo)
    case_sensitive = args.case_sensitive or not args.ignore_case

    env = os.environ.copy()
    env["GFR_SEARCH"] = args.query
    env["GFR_CASE"] = "1" if case_sensitive else "0"
    env["GFR_PEOPLE_ONLY"] = "1" if args.people_only else "0"
    env["GFR_MESSAGE_ONLY"] = "1" if args.message_only else "0"

    cmd = [
        filter_repo,
        "--force",
        "--dry-run",
        "--commit-callback",
        CALLBACK,
    ]
    proc = subprocess.run(
        cmd,
        cwd=str(repo),
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    # Matches come from print() in the callback (stdout). Progress stays on stderr.
    if proc.stderr:
        for err_line in proc.stderr.splitlines():
            # Skip noisy progress; keep real errors.
            if err_line.startswith("Parsed ") and " commits" in err_line:
                continue
            if err_line.strip():
                print(err_line, file=sys.stderr)

    matches: list[str] = []
    for raw in (proc.stdout or "").splitlines():
        m = MATCH_RE.search(raw)
        if m:
            matches.append(m.group("line"))

    if proc.returncode != 0 and not matches:
        sys.stderr.write(proc.stderr or proc.stdout or f"exit {proc.returncode}\n")
        return proc.returncode or 1

    authors: Counter[str] = Counter()
    for line in matches:
        try:
            author = line.split("\tauthor=", 1)[1].split("\tcommitter=", 1)[0]
        except IndexError:
            continue
        authors[author] += 1

    if not args.authors_only:
        for line in matches:
            print(line)

    if args.summary or args.authors_only:
        if matches and not args.authors_only:
            print()
        print(f"matches: {len(matches)}")
        print("by author:")
        for author, count in authors.most_common():
            print(f"  {count:4d}  {author}")

    if not matches:
        print(f"no commits matched {args.query!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
