#!/usr/bin/env python3
"""Turn the targets and releases of cim manifest repositories into a swimflow tree.

A cim manifest repository holds one ``sdk.yml`` per target, and a target may
``extends:`` another one. A release of a target is a git branch of the
repository named ``<target>-<version>`` (``<target>-v1.2.0``), the branches
``cim list-targets -t <target>`` reports.

With one source, the tree is rooted at the manifest repository itself
(yellow). With several, an artificial root (``--root``, default
``manifests``) gets one subtree per repository, in the order given. Below a
repository, each target (blue) hangs under the target it extends, and its
releases (green, dashed connectors) are gathered under a ``releases`` node::

    ./cim-tree.py path/to/manifests -o targets.tree
    ./cim-tree.py first/ second/ -o all.tree
    swimflow targets.tree --bundle targets.html --embed-font

A source is a local folder or the URL of a git repository (``https://``,
``ssh://``, ``git@host:path``, ``file://``). A URL is read as upstream has it:
the manifests come from a shallow clone of its default branch (or of
``URL#BRANCH``) into a temporary directory, and the releases from
``git ls-remote``. Local copies may lag behind upstream; a URL does not::

    ./cim-tree.py https://example.com/org/manifests.git -o targets.tree
    ./cim-tree.py https://example.com/org/manifests.git#develop local/ -o all.tree

The branches are read from the cached refs of the remote ``origin``
(``--remote``; run ``git fetch`` first for fresh ones), or from the remote
itself with ``--live``. A repository without that remote uses its local
branches. A branch belongs to the longest target name it starts with, so
``base-extra-v1.0`` is a release of a target ``base-extra``, not of a target
``base``, when both exist. Only versions matching ``--version-pattern`` (default
``v[0-9]*``) count as releases.

``--match`` keeps only the targets whose name matches one of the given
shell-style patterns (``*``, ``?``, ``[...]``; an exact name is a pattern
too) and ``--exclude`` drops matching targets. ``--match-version`` and
``--exclude-version`` do the same for the versions, and ``--no-releases``
leaves them out. All of them take a comma-separated list and can be
repeated::

    ./cim-tree.py manifests -m 'base*' -X '*-rc.*' -o some.tree

A kept target that extends a dropped one hangs under the nearest kept
ancestor (or the root), so filtering never hides a target.

The YAML is read with PyYAML (``requirements.txt``)::

    python3 -m venv scripts/.venv
    scripts/.venv/bin/pip install -r scripts/requirements.txt
    scripts/.venv/bin/python scripts/cim-tree.py path/to/manifests -o targets
"""

import argparse
import fnmatch
import os
import re
import subprocess
import sys
import tempfile

try:
    import yaml
except ImportError:
    HERE = os.path.dirname(os.path.abspath(__file__))
    sys.exit(
        "error: PyYAML is not installed. Set up the virtual environment once:\n"
        f"  python3 -m venv {HERE}/.venv\n"
        f"  {HERE}/.venv/bin/pip install -r {HERE}/requirements.txt\n"
        f"and run the script with {HERE}/.venv/bin/python (or activate the venv)."
    )

MANIFEST_NAME = "sdk.yml"
ROOT_COLOR = "#fff2a8"
TARGET_COLOR = "#cfe8ff"
RELEASE_COLOR = "#d9f2d9"


# ---------------------------------------------------------------------------
# Manifests
# ---------------------------------------------------------------------------


def find_targets(folder):
    """``{target: extends}`` for every ``sdk.yml`` below `folder`."""
    targets, paths = {}, {}
    for root, dirs, files in os.walk(folder):
        dirs[:] = sorted(d for d in dirs if not d.startswith("."))
        if MANIFEST_NAME not in files:
            continue
        path = os.path.join(root, MANIFEST_NAME)
        name = os.path.basename(os.path.abspath(root))
        if name in targets:
            raise SystemExit(
                f"error: two targets are called {name!r}: {paths[name]} and {path}"
            )
        try:
            with open(path, encoding="utf-8") as handle:
                data = yaml.safe_load(handle)
        except yaml.YAMLError as err:
            raise SystemExit(f"error: {path}: {err}")
        extends = data.get("extends") if isinstance(data, dict) else None
        targets[name] = str(extends) if isinstance(extends, (str, int, float)) else None
        paths[name] = path
    return targets, paths


# ---------------------------------------------------------------------------
# Branches
# ---------------------------------------------------------------------------


def git(folder, *args):
    result = subprocess.run(
        ["git", "-C", folder, *args], capture_output=True, text=True, check=False
    )
    return result.stdout.splitlines() if result.returncode == 0 else None


def branches(folder, remote, live):
    """The branch names of the repository holding `folder`, and a warning or None."""
    if git(folder, "rev-parse", "--git-dir") is None:
        return [], f"{folder} is not a git repository; no releases"
    if remote in (git(folder, "remote") or []):
        if live:
            heads = git(folder, "ls-remote", "--heads", remote)
            if heads is not None:
                return [h.split("\trefs/heads/", 1)[-1] for h in heads if h], None
            warning = f"{folder}: cannot reach {remote!r}; using its cached refs"
        else:
            warning = None
        refs = git(folder, "for-each-ref", f"refs/remotes/{remote}", "--format=%(refname)")
        prefix = f"refs/remotes/{remote}/"
        return [r[len(prefix):] for r in refs or [] if r != prefix + "HEAD"], warning
    refs = git(folder, "for-each-ref", "refs/heads", "--format=%(refname)")
    return [r[len("refs/heads/"):] for r in refs or []], None


URL_PREFIXES = ("https://", "http://", "ssh://", "git://", "file://")


def is_url(source):
    return source.startswith(URL_PREFIXES) or re.match(r"^[\w.-]+@[\w.-]+:", source)


def split_url(source):
    """``(url, branch or None)`` of ``URL`` or ``URL#BRANCH``."""
    url, _, branch = source.partition("#")
    return url, branch or None


def repo_name(source):
    """The name of a repository: the last path part, without ``.git``."""
    name = re.split(r"[/:]", split_url(source)[0].rstrip("/"))[-1]
    return name[:-4] if name.endswith(".git") else name


def clone(url, branch, into):
    """A shallow clone of `url` (at `branch`) into `into`, or an error message."""
    args = ["git", "clone", "--quiet", "--depth", "1", "--single-branch"]
    if branch:
        args += ["--branch", branch]
    result = subprocess.run(args + [url, into], capture_output=True, text=True, check=False)
    if result.returncode != 0:
        return f"cannot clone {url}: {result.stderr.strip()}"
    return None


def remote_branches(url):
    """The branch names of the repository at `url`, or None."""
    heads = git(".", "ls-remote", "--heads", url)
    if heads is None:
        return None
    return [h.split("\trefs/heads/", 1)[-1] for h in heads if h]


def releases(targets, names, version_pattern, match, exclude):
    """``{target: [version]}``: each branch goes to the longest target it starts with."""
    result = {}
    by_length = sorted(targets, key=len, reverse=True)
    for branch in names:
        for target in by_length:
            if branch.startswith(target + "-"):
                version = branch[len(target) + 1:]
                if fnmatch.fnmatchcase(version, version_pattern) and selected(
                    version, match, exclude
                ):
                    result.setdefault(target, []).append(version)
                break
    return {t: sorted(set(v), key=natural) for t, v in result.items()}


def natural(text):
    """A sort key that orders ``v0.10.0`` after ``v0.9.0``."""
    return [(0, int(part), "") if part.isdigit() else (1, 0, part)
            for part in re.split(r"(\d+)", text)]


# ---------------------------------------------------------------------------
# Filtering
# ---------------------------------------------------------------------------


def patterns(values):
    """The patterns of repeated, comma-separated pattern options."""
    return [p.strip() for value in values or [] for p in value.split(",") if p.strip()]


def selected(name, match, exclude):
    if match and not any(fnmatch.fnmatchcase(name, p) for p in match):
        return False
    return not any(fnmatch.fnmatchcase(name, p) for p in exclude)


def parents(targets, match, exclude):
    """``{kept target: kept parent or None}``.

    A target whose parent is dropped (or unknown) gets the nearest kept
    ancestor.
    """
    keep = {name for name in targets if selected(name, match, exclude)}
    result = {}
    for name in keep:
        seen, parent = {name}, targets[name]
        while parent in targets and parent not in keep and parent not in seen:
            seen.add(parent)
            parent = targets[parent]
        result[name] = parent if parent in keep and parent != name else None
    return result


# ---------------------------------------------------------------------------
# Tree source
# ---------------------------------------------------------------------------


def quote(text):
    return '"' + str(text).replace("\\", "\\\\").replace('"', '\\"') + '"'


def target_lines(parent_of, versions, indent):
    """The target nodes of one repository, each line starting with `indent`."""
    out = []
    dashed = f'color = {quote(RELEASE_COLOR)}, edgestyle = dashed'

    def children(parent):
        # A cycle of extends has no root; such targets hang off the root.
        return sorted(t for t, p in parent_of.items() if p == parent)

    def emit(target, indent, seen):
        seen.add(target)
        kids = [k for k in children(target) if k not in seen]
        own = versions.get(target, [])
        head = f"{indent}{quote(target)} [color = {quote(TARGET_COLOR)}]"
        if not kids and not own:
            out.append(head + ";")
            return
        out.append(head + " {")
        for kid in kids:
            emit(kid, indent + "  ", seen)
        if own:
            out.append(f"{indent}  releases [{dashed}] {{")
            out.extend(f"{indent}    {quote(v)} [{dashed}];" for v in own)
            out.append(f"{indent}  }}")
        out.append(indent + "}")

    seen = set()
    for target in children(None):
        emit(target, indent, seen)
    for target in sorted(set(parent_of) - seen):
        emit(target, indent, seen)
    return out


def repository(name):
    """The attributes of a node standing for a manifest repository."""
    return f"[shape = roundedbox, color = {quote(ROOT_COLOR)}]"


def document(repositories, root):
    """The ``tree`` block of ``[(name, parent_of, versions)]``.

    Without `root`, the single repository is the root itself. With it, the
    repositories hang, in the order given, under an artificial root `root`.
    """
    if root is None:
        name, parent_of, versions = repositories[0]
        return ([f"tree {quote(name)} {{", f"  root {quote(name)} {repository(name)};"]
                + target_lines(parent_of, versions, "  ") + ["}"])
    out = [f"tree {quote(root)} {{", f"  root {quote(root)} [shape = roundedbox];"]
    for name, parent_of, versions in repositories:
        body = target_lines(parent_of, versions, "    ")
        head = f"  {quote(name)} {repository(name)}"
        out += [head + " {", *body, "  }"] if body else [head + ";"]
    return out + ["}"]


def read_source(parser, args, source, url, folder, warnings):
    """``(name, parent_of, versions)`` of one source, or None when all is filtered out."""
    match, exclude = patterns(args.match), patterns(args.exclude)
    vmatch, vexclude = patterns(args.match_version), patterns(args.exclude_version)
    targets, paths = find_targets(folder)
    if not targets:
        parser.error(f"no {MANIFEST_NAME} found in {source}")
    for name, extends in sorted(targets.items()):
        if extends and extends not in targets:
            where = os.path.relpath(paths[name], folder) if url else paths[name]
            warnings.append(f"{source}: {where}: extends {extends!r}, which was not "
                            "found; shown under the root")
    parent_of = parents(targets, match, exclude)
    if not parent_of:
        return None
    versions = {}
    if not args.no_releases:
        if url:
            names = remote_branches(url)
            if names is None:
                names = []
                warnings.append(f"cannot list the branches of {url}; no releases")
        else:
            names, warning = branches(folder, args.remote, args.live)
            if warning:
                warnings.append(warning)
        # Every target claims its branches, kept or not, so a dropped
        # `base-extra` does not hand its releases to a kept `base`.
        versions = releases(targets, names, args.version_pattern, vmatch, vexclude)
    name = repo_name(source) if url else os.path.basename(os.path.abspath(folder))
    return name, parent_of, versions


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Write the targets and releases of cim manifest repositories "
        "as a swimflow tree (.tree).",
        epilog="Each source is searched recursively for sdk.yml files and becomes one "
        "tree; a release is a git branch named <target>-<version>. A URL is cloned "
        "shallowly into a temporary directory and its branches listed with "
        "git ls-remote.",
    )
    parser.add_argument("sources", nargs="+", metavar="SOURCE",
                        help="a manifest repository: a local folder, or a git URL "
                        "(optionally URL#BRANCH)")
    parser.add_argument("--root", default="manifests",
                        help="the name of the common root above several sources "
                        "(default: %(default)s)")
    parser.add_argument("-o", "--output", metavar="FILE",
                        help="the .tree file to write (default: standard output); "
                        "'.tree' is added when FILE has no extension")
    parser.add_argument("--remote", default="origin",
                        help="for a local folder, the remote whose branches are releases "
                        "(default: %(default)s); local branches are used when the "
                        "repository has no such remote")
    parser.add_argument("--live", action="store_true",
                        help="for a local folder, ask the remote for its branches "
                        "(git ls-remote) instead of using the cached refs")
    parser.add_argument("--version-pattern", default="v[0-9]*", metavar="PATTERN",
                        help="what the <version> of a release branch looks like "
                        "(default: %(default)s; '*' takes every branch)")
    parser.add_argument("--no-releases", action="store_true", help="leave the releases out")
    parser.add_argument("-m", "--match", action="append", metavar="PATTERNS",
                        help="keep only targets matching one of these comma-separated "
                        "patterns (* ? [...]); can be repeated")
    parser.add_argument("-x", "--exclude", action="append", metavar="PATTERNS",
                        help="drop targets matching one of these comma-separated "
                        "patterns; can be repeated")
    parser.add_argument("-M", "--match-version", action="append", metavar="PATTERNS",
                        help="keep only versions matching one of these patterns")
    parser.add_argument("-X", "--exclude-version", action="append", metavar="PATTERNS",
                        help="drop versions matching one of these patterns")
    args = parser.parse_args(argv)
    out, warnings = [], []
    with tempfile.TemporaryDirectory(prefix="cim-tree-") as scratch:
        for index, source in enumerate(args.sources):
            if is_url(source):
                url, branch = split_url(source)
                folder = os.path.join(scratch, str(index))
                error = clone(url, branch, folder)
                if error:
                    parser.error(error)
            else:
                url, folder = None, source
                if not os.path.isdir(folder):
                    parser.error(f"{folder} is not a directory")
            found = read_source(parser, args, source, url, folder, warnings)
            if found:
                out.append(found)

    if not out:
        parser.error("no target matches --match/--exclude")
    # Several sources share an artificial root, even when the filters keep
    # only one of them.
    out = document(out, args.root if len(args.sources) > 1 else None)
    header = [
        "// Generated by scripts/cim-tree.py.",
        "//",
        "// Solid connector: a target, under the target it extends. Dashed connector:",
        "// a release, i.e. a git branch named <target>-<version>.",
        "",
    ]
    source = "\n".join(header + out) + "\n"
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    if args.output:
        path = args.output if os.path.splitext(args.output)[1] else args.output + ".tree"
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(source)
    else:
        sys.stdout.write(source)
    return 0


if __name__ == "__main__":
    sys.exit(main())
