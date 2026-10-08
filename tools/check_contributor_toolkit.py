"""Check contributor Markdown links, skill metadata and catalog coverage."""

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"\[[^\]\n]+\]\(([^)\s]+)\)")


def check(root):
    errors = []
    toolkit = root / ".agents"
    catalog_path = toolkit / "README.md"
    if not catalog_path.is_file():
        return ["missing .agents/README.md"]
    catalog = catalog_path.read_text(encoding="utf-8")
    documents = sorted(toolkit.rglob("*.md"))
    documents += [root / name for name in ("AGENTS.md", "CONTRIBUTING.md", "README.md")]
    for path in documents:
        label = path.relative_to(root)
        if not path.is_file():
            errors.append(f"missing {label}")
            continue
        source = path.read_text(encoding="utf-8")
        # Fenced examples may contain illustrative links rather than references.
        prose = re.sub(r"(?ms)^```[^\n]*\n.*?^```[^\n]*$", "", source)
        for target in LINK.findall(prose):
            url = urlsplit(target)
            if url.scheme or url.netloc or not url.path:
                continue
            resolved = (path.parent / unquote(url.path)).resolve()
            if not resolved.is_relative_to(root.resolve()) or not resolved.exists():
                errors.append(f"{label}: broken local link {target}")
        relative = path.relative_to(toolkit) if path.is_relative_to(toolkit) else None
        if relative and relative.parts[0] in ("skills", "agents", "commands"):
            metadata = re.match(r"\A---\n(.*?)\n---\n", source, re.DOTALL)
            if not metadata:
                errors.append(f"{label}: missing front matter")
                continue
            fields = dict(re.findall(r"^(name|description):\s*(\S[^\n]*)$", metadata[1], re.MULTILINE))
            expected_name = path.parent.name if path.name == "SKILL.md" else path.stem
            if fields.get("name") != expected_name or not fields.get("description"):
                errors.append(f"{label}: name must match path and description must be nonempty")
            if relative.as_posix() not in catalog:
                errors.append(f"{label}: absent from toolkit catalog")
    if not list((toolkit / "skills").glob("*/SKILL.md")):
        errors.append("no contributor skills found")
    return errors


def main():
    errors = check(ROOT)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Contributor toolkit links, metadata and catalog coverage passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
