"""Check contributor Markdown links, skill metadata and catalog coverage."""

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

import yaml
from markdown_it import MarkdownIt


ROOT = Path(__file__).resolve().parents[1]
MARKDOWN = MarkdownIt("commonmark")
FRONTMATTER = re.compile(r"\A---\n(.*?)\n---(?:\n|\Z)", re.DOTALL)


def local_links(path, source, *, images=True):
    frontmatter = FRONTMATTER.match(source)
    body = source[frontmatter.end():] if frontmatter else source
    for block in MARKDOWN.parse(body):
        for token in block.children or []:
            if token.type == "link_open":
                target = token.attrGet("href")
            elif images and token.type == "image":
                target = token.attrGet("src")
            else:
                continue
            if not target:
                continue
            url = urlsplit(target)
            if not url.scheme and not url.netloc and url.path:
                yield target, (path.parent / unquote(url.path)).resolve()


def metadata_fields(source):
    try:
        metadata = yaml.compose(source, Loader=yaml.SafeLoader)
    except yaml.YAMLError:
        return None
    if not isinstance(metadata, yaml.MappingNode):
        return None
    fields = {}
    for key, value in metadata.value:
        if (not isinstance(key, yaml.ScalarNode) or key.tag != "tag:yaml.org,2002:str"
                or key.value in fields):
            return None
        fields[key.value] = value
    result = {}
    for name in ("name", "description"):
        value = fields.get(name)
        if not isinstance(value, yaml.ScalarNode) or value.tag != "tag:yaml.org,2002:str":
            return None
        result[name] = value.value
    return result


def check(root):
    errors = []
    toolkit = root / ".agents"
    catalog_path = toolkit / "README.md"
    if not catalog_path.is_file():
        return ["missing .agents/README.md"]
    catalog = catalog_path.read_text(encoding="utf-8")
    catalog_targets = {resolved for _, resolved in local_links(catalog_path, catalog, images=False)}
    documents = sorted(toolkit.rglob("*.md"))
    documents += [root / name for name in ("AGENTS.md", "CONTRIBUTING.md", "README.md")]
    for path in documents:
        label = path.relative_to(root)
        if not path.is_file():
            errors.append(f"missing {label}")
            continue
        source = path.read_text(encoding="utf-8")
        for target, resolved in local_links(path, source):
            if not resolved.is_relative_to(root.resolve()) or not resolved.exists():
                errors.append(f"{label}: broken local link {target}")
        relative = path.relative_to(toolkit) if path.is_relative_to(toolkit) else None
        if relative and relative.parts[0] in ("skills", "agents", "commands"):
            metadata = FRONTMATTER.match(source)
            if not metadata:
                errors.append(f"{label}: missing front matter")
                continue
            fields = metadata_fields(metadata[1])
            expected_name = path.parent.name if path.name == "SKILL.md" else path.stem
            if (not fields or fields["name"] != expected_name or not fields["description"].strip()):
                errors.append(f"{label}: name must match path and description must be nonempty; "
                              "use a YAML mapping with unique fields and string values")
            if path.resolve() not in catalog_targets:
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
