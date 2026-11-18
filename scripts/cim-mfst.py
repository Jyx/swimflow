#!/usr/bin/env python3
"""Turn the gits of a cim manifest repository into a swimflow manifest (.mfst).

A cim manifest repository holds one ``sdk.yml`` per target. Each target lists
the git repositories it is made of under ``gits:``, and a target may
``extends:`` another one: it then inherits the parent's gits, and its
``overlay: gits:`` section can ``remove`` some of them and ``modify`` others.

The script writes one manifest column per target and one row per git name. A
column that extends another one gets ``[extends = PARENT]``, removed gits
become ``remove NAME;`` and modified gits ``override NAME [note = "..."];``, so
swimflow draws direct, inherited, removed and overridden gits differently.

Every manifest folder given on the command line becomes one column group,
named after the folder::

    ./cim-mfst.py path/to/manifests -o gits.mfst
    ./cim-mfst.py first/ second/ --title "Gits per target" -o all.mfst
    swimflow gits.mfst --bundle gits.html --embed-font

``--match`` keeps only the targets whose name matches one of the given
shell-style patterns (``*``, ``?``, ``[...]``; an exact name is a pattern
too) and ``--exclude`` drops matching targets. Both take a comma-separated
list and can be repeated::

    ./cim-mfst.py manifests -m base,product-a -o some.mfst
    ./cim-mfst.py manifests -x 'test-*' -o most.mfst

A kept target that extends a dropped one inherits the dropped target's gits
directly, so filtering never hides a git from a column.

A git ``name`` may hold a path (``name: ws/zephyr`` checks the git out in a
subfolder); the row is named after the part after the last ``/``, so the same
git checked out in different places is one row. ``--full-names`` keeps the
whole name::

    ./cim-mfst.py first/ second/ --full-names -o all.mfst

The YAML is read with PyYAML (``requirements.txt``)::

    python3 -m venv scripts/.venv
    scripts/.venv/bin/pip install -r scripts/requirements.txt
    scripts/.venv/bin/python scripts/cim-mfst.py path/to/manifests -o gits
"""

import argparse
import copy
import fnmatch
import os
import sys

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


class ManifestError(Exception):
    """A manifest file that cannot be read."""


def load_yaml(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return yaml.safe_load(handle)
    except yaml.YAMLError as err:
        raise ManifestError(str(err)) from err


# ---------------------------------------------------------------------------
# Manifests -> manifest
# ---------------------------------------------------------------------------


class Target:
    def __init__(self, name, path, data, full_names=False):
        self.name = name
        self.path = path
        data = data if isinstance(data, dict) else {}
        self.extends = data.get("extends")
        self.gits = names(data.get("gits"), full_names)
        overlay = data.get("overlay") or {}
        overlay_gits = overlay.get("gits") if isinstance(overlay, dict) else None
        overlay_gits = overlay_gits if isinstance(overlay_gits, dict) else {}
        self.removed = names(overlay_gits.get("remove"), full_names)
        self.modified = []
        for entry in overlay_gits.get("modify") or []:
            if isinstance(entry, dict) and entry.get("name"):
                changes = ", ".join(
                    f"{key}: {value}" for key, value in entry.items() if key != "name"
                )
                self.modified.append((git_name(entry["name"], full_names), changes))


def git_name(name, full_names=False):
    """The row name of a git: the part after the last `/` unless `full_names`."""
    name = str(name)
    if full_names:
        return name
    return name.rstrip("/").rsplit("/", 1)[-1] or name


def names(items, full_names=False):
    """The names of a list of gits: mappings with a `name`, or plain strings."""
    result = []
    for item in items or []:
        if isinstance(item, dict) and item.get("name"):
            result.append(git_name(item["name"], full_names))
        elif isinstance(item, str):
            result.append(git_name(item, full_names))
    return list(dict.fromkeys(result))


def find_targets(folder, full_names=False):
    """Every ``sdk.yml`` below `folder`, named after its directory."""
    targets = {}
    for root, dirs, files in os.walk(folder):
        dirs[:] = sorted(d for d in dirs if not d.startswith("."))
        if MANIFEST_NAME not in files:
            continue
        path = os.path.join(root, MANIFEST_NAME)
        name = os.path.basename(os.path.abspath(root))
        if name in targets:
            raise SystemExit(
                f"error: two targets are called {name!r}: {targets[name].path} and {path}"
            )
        try:
            targets[name] = Target(name, path, load_yaml(path), full_names)
        except ManifestError as err:
            raise SystemExit(f"error: {path}: {err}")
    return targets


def ordered(targets):
    """Base targets in name order, each followed by the targets extending it."""
    order = []

    def visit(name):
        if name in order:
            return
        order.append(name)
        for child in sorted(t for t, target in targets.items() if target.extends == name):
            visit(child)

    for name in sorted(t for t, target in targets.items() if target.extends not in targets):
        visit(name)
    # A cycle of extends leaves targets unvisited; keep them, in name order.
    order.extend(sorted(set(targets) - set(order)))
    return order


def patterns(values):
    """The patterns of repeated, comma-separated ``--match``/``--exclude`` values."""
    return [p.strip() for value in values or [] for p in value.split(",") if p.strip()]


def selected(name, match, exclude):
    if match and not any(fnmatch.fnmatchcase(name, p) for p in match):
        return False
    return not any(fnmatch.fnmatchcase(name, p) for p in exclude)


def filtered(targets, match, exclude):
    """The targets selected by `match`/`exclude`.

    A kept target whose parent is dropped gets the gits of the dropped
    ancestors as its own (minus what it removes), and extends the nearest
    kept ancestor, if any.
    """
    keep = {name for name in targets if selected(name, match, exclude)}
    result = {}
    for name in keep:
        target = targets[name]
        chain, parent = [], target.extends
        while parent in targets and parent not in keep and targets[parent] not in chain:
            chain.append(targets[parent])
            parent = targets[parent].extends
        if not chain:
            result[name] = target
            continue
        inherited, removed = [], []
        for ancestor in reversed(chain):
            inherited = [g for g in inherited if g not in ancestor.removed]
            inherited += [g for g in ancestor.gits if g not in inherited]
            removed += ancestor.removed
        target = copy.copy(target)
        target.gits = [g for g in inherited if g not in target.removed] + [
            g for g in target.gits if g not in inherited
        ]
        if parent in keep:
            target.extends = parent
            target.removed = list(dict.fromkeys(removed + target.removed))
        else:
            target.extends = None
            target.removed = []
            target.modified = []
        result[name] = target
    return result


def quote(text):
    return '"' + str(text).replace("\\", "\\\\").replace('"', '\\"') + '"'


def manifest(groups, title):
    """The .mfst source for ``[(group name, {target: Target})]``.

    Column names must be unique in a manifest. With one folder a column is the
    target name; with several, two folders may share a target name, so the
    column is ``FOLDER/TARGET`` and the target name is its label. A target
    extends a target of its own folder.
    """
    qualify = len(groups) > 1

    def column(group, name):
        return f"{group}/{name}" if qualify else name

    out = [f"manifest {quote(title)} {{", f"  title = {quote(title)};", '  countlabel = "targets";']
    warnings = []
    for group, targets in groups:
        out.append(f"  group {quote(group)} {{")
        for name in ordered(targets):
            target = targets[name]
            attrs = [f"label = {quote(name)}"] if qualify else []
            inherits = bool(target.extends) and target.extends in targets
            if inherits:
                attrs.append(f"extends = {quote(column(group, target.extends))}")
            elif target.extends:
                warnings.append(
                    f"{target.path}: extends {target.extends!r}, which was not found; "
                    "its inherited gits are not shown"
                )
            entries = [f"      {quote(git)};" for git in target.gits]
            if inherits:
                entries += [f"      remove {quote(git)};" for git in target.removed]
                entries += [
                    f"      override {quote(git)} [note = {quote(note)}];" if note
                    else f"      override {quote(git)};"
                    for git, note in target.modified
                ]
            head = f"    column {quote(column(group, name))}"
            if attrs:
                head += f" [{', '.join(attrs)}]"
            if entries:
                out.append(head + " {")
                out.extend(entries)
                out.append("    }")
            else:
                out.append(head + ";")
        out.append("  }")
    out.append("}")
    return "\n".join(out) + "\n", warnings


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Write the gits of cim manifest targets as a swimflow manifest (.mfst).",
        epilog="Each folder is searched recursively for sdk.yml files; every folder "
        "becomes one column group and every target one column.",
    )
    parser.add_argument("folders", nargs="+", metavar="FOLDER", help="a manifest repository")
    parser.add_argument("-o", "--output", metavar="FILE",
                        help="the .mfst file to write (default: standard output); "
                        "'.mfst' is added when FILE has no extension")
    parser.add_argument("--title", default="Gits per target",
                        help="the manifest title (default: %(default)s)")
    parser.add_argument("-m", "--match", action="append", metavar="PATTERNS",
                        help="keep only targets matching one of these comma-separated "
                        "patterns (* ? [...]); can be repeated")
    parser.add_argument("-x", "--exclude", action="append", metavar="PATTERNS",
                        help="drop targets matching one of these comma-separated "
                        "patterns; can be repeated")
    parser.add_argument("--full-names", action="store_true",
                        help="keep the whole git name, path included (default: the "
                        "part after the last '/', so 'ws/zephyr' is the row 'zephyr')")
    args = parser.parse_args(argv)
    match, exclude = patterns(args.match), patterns(args.exclude)

    groups = []
    for folder in args.folders:
        if not os.path.isdir(folder):
            parser.error(f"{folder} is not a directory")
        targets = find_targets(folder, args.full_names)
        if not targets:
            parser.error(f"no {MANIFEST_NAME} found below {folder}")
        targets = filtered(targets, match, exclude)
        if not targets:
            continue
        groups.append((os.path.basename(os.path.abspath(folder)), targets))

    if not groups:
        parser.error("no target matches --match/--exclude")
    source, warnings = manifest(groups, args.title)
    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)
    if args.output:
        path = args.output if os.path.splitext(args.output)[1] else args.output + ".mfst"
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(source)
    else:
        sys.stdout.write(source)
    return 0


if __name__ == "__main__":
    sys.exit(main())
