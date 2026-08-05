#!/usr/bin/env python3
"""AOKF 0.1 reference validator.

Runs the document check of SPEC.md section 10 over a bundle and grades it
against the conformance ladder of section 11:

  Level 0  every non-reserved .md passes the document check.
  Level 1  Level 0 + every concept has a unique `id` + a manifest
           declaring `aokf` and `name`.
  Level 2  Level 1 + every `links` entry has a valid `rel` and a `to`
           that resolves, and is mirrored by a body link.

Usage:
    python3 validator.py <bundle-dir> [--level N] [--json] [--repo-root DIR]

Exit codes: 0 pass at the checked level, 1 fail, 2 usage error.

Findings below the checked level print as warnings; the spec's "warn on"
items (section 10 item 5) that the ladder never hardens stay warnings at
every level. Warnings never fail the run.

Self-contained: uses PyYAML if present, otherwise a minimal built-in parser
covering the subset of YAML that AOKF frontmatter uses.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime
from pathlib import Path

MANIFEST_NAME = "manifest.aokf.yaml"
RESERVED_MD = {"index.md"}

CORE_RELS = {
    "relates-to", "part-of", "has-part", "depends-on", "depended-on-by",
    "references", "referenced-by", "supersedes", "superseded-by",
    "contradicts",
}

SLUG_RE = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")
ACTOR_RE = re.compile(r"^(human|process):.+")
FM_RE = re.compile(r"\A---\s*\n(.*?)\n---\s*(?:\n|\Z)", re.DOTALL)
MD_LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")
FOOTNOTE_RE = re.compile(r"\[\^([^\]]+)\]")

STAMPED_CONCEPT = ("generated",)
STAMPED_MANIFEST = ("producer", "generated", "counts")

# --- YAML loading (PyYAML if available, else a minimal fallback) -------------

try:
    import yaml  # type: ignore

    def load_yaml(text: str):
        return yaml.safe_load(text)
except Exception:  # pragma: no cover - fallback path
    def load_yaml(text: str):
        return _mini_yaml(text)


def _coerce(v: str):
    s = v.strip()
    if s == "" or s == "~" or s.lower() == "null":
        return None
    if s.lower() in ("true", "false"):
        return s.lower() == "true"
    if re.fullmatch(r"-?\d+", s):
        return int(s)
    if (s.startswith('"') and s.endswith('"')) or (s.startswith("'") and s.endswith("'")):
        return s[1:-1]
    if s.startswith("[") and s.endswith("]"):
        inner = s[1:-1].strip()
        return [_coerce(x) for x in inner.split(",")] if inner else []
    if s.startswith("{") and s.endswith("}"):
        out = {}
        for part in s[1:-1].split(","):
            if ":" in part:
                k, _, val = part.partition(":")
                out[k.strip()] = _coerce(val)
        return out
    return s


def _mini_yaml(text: str):
    """Minimal parser: top-level scalars, flow lists/maps, one level of
    block nesting, and lists-of-maps (as used by `sources`, `links`,
    `verified`). Not a general YAML parser."""
    root: dict = {}
    lines = [ln.rstrip("\n") for ln in text.split("\n")]
    i = 0
    n = len(lines)

    def indent(ln: str) -> int:
        return len(ln) - len(ln.lstrip(" "))

    while i < n:
        ln = lines[i]
        if not ln.strip() or ln.lstrip().startswith("#"):
            i += 1
            continue
        if indent(ln) != 0:
            i += 1
            continue
        key, _, rest = ln.partition(":")
        key = key.strip()
        rest = rest.strip()
        if rest:
            root[key] = _coerce(rest)
            i += 1
            continue
        block = []
        j = i + 1
        while j < n and (not lines[j].strip() or indent(lines[j]) > 0):
            block.append(lines[j])
            j += 1
        root[key] = _parse_block(block)
        i = j
    return root


def _parse_block(block: list[str]):
    items = [ln for ln in block if ln.strip() and not ln.lstrip().startswith("#")]
    if not items:
        return None
    base = min(len(ln) - len(ln.lstrip(" ")) for ln in items)
    is_list = all(
        ln.lstrip().startswith("- ") or ln.strip() == "-"
        for ln in items
        if (len(ln) - len(ln.lstrip(" "))) == base
    )
    if is_list:
        result = []
        cur = None
        for ln in items:
            ind = len(ln) - len(ln.lstrip(" "))
            body = ln.lstrip()
            if ind == base and body.startswith("-"):
                if cur is not None:
                    result.append(cur)
                body = body[1:].strip()
                if not body:
                    cur = {}
                elif body.startswith("{"):
                    cur = _coerce(body)
                elif ":" in body:
                    k, _, v = body.partition(":")
                    cur = {k.strip(): _coerce(v)}
                else:
                    cur = _coerce(body)
            elif isinstance(cur, dict) and ":" in body:
                k, _, v = body.partition(":")
                cur[k.strip()] = _coerce(v)
        if cur is not None:
            result.append(cur)
        return result
    result = {}
    for ln in items:
        if (len(ln) - len(ln.lstrip(" "))) != base:
            continue
        k, _, v = ln.strip().partition(":")
        result[k.strip()] = _coerce(v.strip())
    return result


# --- Findings ----------------------------------------------------------------


class Finding:
    """error_at: lowest conformance level at which this is an error, or
    None for the spec's always-warn items (section 10 item 5)."""

    def __init__(self, path: str, msg: str, error_at: int | None):
        self.path = path
        self.msg = msg
        self.error_at = error_at

    def severity(self, checked_level: int) -> str:
        if self.error_at is not None and self.error_at <= checked_level:
            return "error"
        return "warning"

    def as_dict(self, checked_level: int):
        return {
            "severity": self.severity(checked_level),
            "error_at_level": self.error_at,
            "file": self.path,
            "message": self.msg,
        }


# --- Validation ----------------------------------------------------------------


def parse_frontmatter(path: Path):
    text = path.read_text(encoding="utf-8")
    m = FM_RE.match(text)
    if not m:
        return None, "no frontmatter block", text
    body = text[m.end():]
    try:
        fm = load_yaml(m.group(1))
    except Exception as e:  # noqa: BLE001
        return None, f"frontmatter YAML parse error: {e}", body
    if not isinstance(fm, dict):
        return None, "frontmatter is not a mapping", body
    return fm, "", body


def is_iso8601(v) -> bool:
    if isinstance(v, datetime):
        return True
    if not isinstance(v, str):
        return False
    try:
        datetime.fromisoformat(v.replace("Z", "+00:00"))
        return True
    except ValueError:
        return False


def resolve_target(target: str, concept_dir: Path, repo_root: Path) -> Path | None:
    """Resolve a body-link or `to` path target to an existing file, or None."""
    t = target.split("#")[0]
    if not t or t.startswith(("http://", "https://", "mailto:")):
        return None
    p = repo_root / t.lstrip("/") if t.startswith("/") else concept_dir / t
    try:
        return p.resolve() if p.exists() else None
    except OSError:
        return None


def validate(bundle: Path, repo_root: Path):
    findings: list[Finding] = []

    def err(path, msg, at=0):
        findings.append(Finding(path, msg, at))

    def warn(path, msg):
        findings.append(Finding(path, msg, None))

    md_files = sorted(p for p in bundle.rglob("*.md") if p.name not in RESERVED_MD)
    index_files = sorted(p for p in bundle.rglob("index.md"))

    # First pass: parse everything, collect ids.
    parsed: dict[str, tuple[dict, str, Path]] = {}
    ids: dict[str, str] = {}
    for p in md_files:
        rel = str(p.relative_to(bundle))
        fm, perr, body = parse_frontmatter(p)
        if fm is None:
            err(rel, perr)
            continue
        parsed[rel] = (fm, body, p)
        cid = fm.get("id")
        if cid is not None:
            cid = str(cid)
            if not SLUG_RE.fullmatch(cid):
                err(rel, f"`id` is not a valid slug: {cid!r}")
            elif cid in ids:
                err(rel, f"duplicate `id` {cid!r} (also in {ids[cid]})")
            else:
                ids[cid] = rel

    # Manifest (document check: parses, no stamped keys; Level 1: exists
    # and declares `aokf` and `name`).
    manifest = bundle / MANIFEST_NAME
    man = None
    if manifest.exists():
        try:
            man = load_yaml(manifest.read_text(encoding="utf-8")) or {}
        except Exception as e:  # noqa: BLE001
            err(MANIFEST_NAME, f"manifest parse error: {e}")
        if isinstance(man, dict):
            for k in STAMPED_MANIFEST:
                if k in man:
                    err(MANIFEST_NAME, f"stamped key `{k}` present in the working tree")
            for k in ("aokf", "name"):
                if not man.get(k):
                    err(MANIFEST_NAME, f"manifest missing `{k}`", at=1)
    else:
        err(MANIFEST_NAME, "no manifest (required at Level 1)", at=1)

    # Per-concept checks.
    for rel, (fm, body, p) in parsed.items():
        # type: required, non-empty.
        typ = fm.get("type")
        if not typ or not str(typ).strip():
            err(rel, "missing or empty required field `type`")

        # id: required at Level 1.
        if fm.get("id") is None:
            err(rel, "no `id` (required at Level 1)", at=1)

        # Stamped fields absent.
        for k in STAMPED_CONCEPT:
            if k in fm:
                err(rel, f"stamped field `{k}` present in the working tree")

        # verified: well-formed (a bare mapping reads as a one-element list).
        v = fm.get("verified")
        if v is not None:
            entries = [v] if isinstance(v, dict) else v
            if not isinstance(entries, list):
                err(rel, "`verified` must be a mapping or a list of mappings")
            else:
                for i, e in enumerate(entries):
                    if not isinstance(e, dict):
                        err(rel, f"verified[{i}] is not a mapping")
                        continue
                    by = e.get("by")
                    if not isinstance(by, str) or not ACTOR_RE.match(by):
                        err(rel, f"verified[{i}].by must be `human:<id>` or `process:<id>`, got {by!r}")
                    if not is_iso8601(e.get("at")):
                        err(rel, f"verified[{i}].at is not ISO 8601: {e.get('at')!r}")

        # Body links: collect resolved targets; warn on broken paths.
        body_resolved: set[Path] = set()
        for raw in MD_LINK_RE.findall(body):
            t = raw.split("#")[0]
            if not t or t.startswith(("http://", "https://", "mailto:")):
                continue
            resolved = resolve_target(t, p.parent, repo_root)
            if resolved is None:
                warn(rel, f"broken body link: {raw}")
            else:
                body_resolved.add(resolved)

        # resource and sources[].resource repo paths exist.
        res = fm.get("resource")
        if isinstance(res, str) and res.startswith("/"):
            if not (repo_root / res.lstrip("/")).exists():
                warn(rel, f"`resource` path does not exist: {res}")
        sources = fm.get("sources") or []
        src_ids: set[str] = set()
        if not isinstance(sources, list):
            err(rel, "`sources` must be a list")
            sources = []
        for i, s in enumerate(sources):
            if not isinstance(s, dict):
                err(rel, f"sources[{i}] is not a mapping")
                continue
            if not s.get("resource"):
                err(rel, f"sources[{i}] missing `resource`")
            if s.get("id") is not None:
                src_ids.add(str(s["id"]))
            r = s.get("resource")
            if isinstance(r, str) and r.startswith("/"):
                if not (repo_root / r.lstrip("/")).exists():
                    warn(rel, f"sources[{i}].resource does not exist: {r}")

        # Footnote labels join into sources[].id.
        for label in sorted(set(FOOTNOTE_RE.findall(body))):
            if label not in src_ids:
                warn(rel, f"footnote [^{label}] has no matching sources[].id")

        # links entries: rel + to (document check); valid rel, resolving to,
        # mirroring body link (errors at Level 2, warnings below it).
        links = fm.get("links")
        if links is None:
            continue
        if not isinstance(links, list):
            err(rel, "`links` must be a list")
            continue
        for i, ln in enumerate(links):
            where = f"links[{i}]"
            if not isinstance(ln, dict):
                err(rel, f"{where} is not a mapping")
                continue
            relv = ln.get("rel")
            to = ln.get("to")
            if not relv:
                err(rel, f"{where} missing `rel`")
            elif not isinstance(relv, str) or not SLUG_RE.fullmatch(relv):
                err(rel, f"{where} `rel` is not lowercase kebab-case: {relv!r}", at=2)
            elif relv not in CORE_RELS:
                warn(rel, f"{where} non-core rel `{relv}` (read as relates-to)")
            if not to:
                err(rel, f"{where} missing `to`")
                continue
            to = str(to)
            # Resolve: id first, then path.
            target_path: Path | None = None
            if to in ids:
                target_path = (bundle / ids[to]).resolve()
            else:
                target_path = resolve_target(to, p.parent, repo_root)
            if target_path is None:
                err(rel, f"{where} `to: {to}` resolves to no concept id or path", at=2)
                continue
            if target_path not in body_resolved:
                err(rel, f"{where} `to: {to}` has no mirroring body link", at=2)

    # index.md entries point at files that exist.
    for idx in index_files:
        rel = str(idx.relative_to(bundle))
        for raw in MD_LINK_RE.findall(idx.read_text(encoding="utf-8")):
            t = raw.split("#")[0]
            if not t or t.startswith(("http://", "https://", "mailto:")):
                continue
            if resolve_target(t, idx.parent, repo_root) is None:
                warn(rel, f"index entry points at missing file: {raw}")

    return findings, len(parsed)


def achieved_level(findings: list[Finding]) -> int:
    level = -1
    for lv in (0, 1, 2):
        if any(f.error_at is not None and f.error_at <= lv for f in findings):
            break
        level = lv
    return level


def find_repo_root(start: Path) -> Path:
    for d in (start, *start.parents):
        if (d / ".git").exists():
            return d
    return start.parent


def main():
    ap = argparse.ArgumentParser(description="Validate an AOKF 0.1 bundle.")
    ap.add_argument("bundle", type=Path, help="path to the bundle directory")
    ap.add_argument("--level", type=int, default=2, choices=[0, 1, 2],
                    help="conformance level to check at (default 2)")
    ap.add_argument("--repo-root", type=Path, default=None,
                    help="repository root for /-rooted paths (default: nearest .git above the bundle)")
    ap.add_argument("--json", action="store_true", help="emit JSON")
    args = ap.parse_args()

    bundle = args.bundle
    if not bundle.is_dir():
        print(f"error: {bundle} is not a directory", file=sys.stderr)
        sys.exit(2)
    repo_root = (args.repo_root or find_repo_root(bundle.resolve())).resolve()

    findings, n = validate(bundle, repo_root)
    level = args.level
    errors = [f for f in findings if f.severity(level) == "error"]
    warnings = [f for f in findings if f.severity(level) == "warning"]
    achieved = achieved_level(findings)

    if args.json:
        print(json.dumps({
            "bundle": str(bundle),
            "concepts": n,
            "checked_level": level,
            "achieved_level": achieved,
            "passed": not errors,
            "findings": [f.as_dict(level) for f in findings],
        }, indent=2))
        sys.exit(0 if not errors else 1)

    print(f"AOKF validator — bundle: {bundle}")
    print(f"  concepts: {n}")
    print(f"  achieved level: {achieved if achieved >= 0 else 'none'}")
    print(f"  checked at level: {level}")
    if not findings:
        print("  ✓ no findings")
    for f in findings:
        sev = f.severity(level)
        mark = "✗" if sev == "error" else "!"
        print(f"  {mark} [{sev}] {f.path}: {f.msg}")
    print(f"\n{'PASS' if not errors else 'FAIL'} at level {level} "
          f"({len(errors)} error(s), {len(warnings)} warning(s))")
    sys.exit(0 if not errors else 1)


if __name__ == "__main__":
    main()
