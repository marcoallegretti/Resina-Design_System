"""Reject a proposed maintenance issue before it is published.

The maintenance lanes propose at most one issue per run. This check
reads the proposed create_issue output and fails when the body does not follow
the lane's issue form, cites a repository path or line range that does not
exist, describes the process that produced it, or repeats an existing title.
"""

import argparse
import http.client
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

LANE_FORMS = {
    "architecture-debt-scout": ".github/ISSUE_TEMPLATE/architecture.yml",
    "continuous-refactoring": ".github/ISSUE_TEMPLATE/maintenance.yml",
    "continuous-simplicity": ".github/ISSUE_TEMPLATE/maintenance.yml",
}

FILE_EXTENSIONS = (
    "cfg|cjs|css|csv|html|ini|js|json|lock|md|png|py|qml|rs|sh|slint|svg|toml|ts|txt|wgsl|yaml|yml"
)
PATH_TOKEN = re.compile(
    r"(?<![\w./-])"
    r"(?P<path>(?:\.?[\w-][\w.-]*/)+[\w.-]*|\.?[\w-][\w.-]*\.(?:" + FILE_EXTENSIONS + r"))"
    r"(?::L?(?P<start>\d+)(?:-L?(?P<end>\d+))?|#L(?P<hstart>\d+)(?:-L(?P<hend>\d+))?)?"
    r"(?![\w/-])"
)
URL = re.compile(r"https?://\S+")
HEADING = re.compile(r"^ {0,3}(#{1,6})[ \t]+(.*?)[ \t#]*$")
FENCE = re.compile(r"^ {0,3}(```|~~~)")

PROCESS_TERMS = [
    (re.compile(r"\bagents?\b|\bagentic\b", re.IGNORECASE), "agent"),
    (re.compile(r"\bAI\b"), "AI"),
    (re.compile(r"\bLLMs?\b", re.IGNORECASE), "LLM"),
    (re.compile(r"\b(?:language|AI|ML|LLM|foundation|frontier)[ -]models?\b", re.IGNORECASE), "language model"),
    (
        re.compile(
            r"\b(?:copilot|claude|anthropic|openai|chatgpt|gpt-?\d[\w.-]*|gpt|gemini|sonnet|opus|haiku|codex|llama|mistral)\b",
            re.IGNORECASE,
        ),
        "model or vendor name",
    ),
    (re.compile(r"\bgh[- ]aw\b", re.IGNORECASE), "gh-aw"),
    (re.compile(r"\bautomation\b|\bautomated (?:issue|report|review|run|scan|finding)s?\b", re.IGNORECASE), "automation"),
    (
        re.compile(
            r"\b(?:this|workflow|maintenance|scheduled) run\b"
            r"|\b(?:maintenance|scheduled|agentic) workflows?\b"
            r"|\bself-review\b",
            re.IGNORECASE,
        ),
        "description of the issue-filing process",
    ),
]


def form_labels(form_path):
    """Return the textarea and input labels of a GitHub issue form, in order."""
    labels = []
    field_type = None
    for line in Path(form_path).read_text(encoding="utf-8").splitlines():
        item = re.match(r"^\s*-\s+type:\s*(\S+)\s*$", line)
        if item:
            field_type = item.group(1)
            continue
        label = re.match(r"^\s+label:\s*(.+?)\s*$", line)
        if label and field_type in {"textarea", "input", "dropdown"}:
            labels.append(label.group(1).strip("'\""))
            field_type = None
    if not labels:
        raise ValueError(f"{form_path} declares no labelled fields")
    return labels


def title_prefix(workflow_source):
    match = re.search(r"^\s+title-prefix:\s*\"([^\"]*)\"\s*$", Path(workflow_source).read_text(encoding="utf-8"), re.MULTILINE)
    if not match:
        raise ValueError(f"{workflow_source} declares no quoted create-issue title-prefix")
    return match.group(1)


def lane_from_workflow_ref(workflow_ref):
    path = workflow_ref.split("@", 1)[0]
    name = path.rsplit("/", 1)[-1]
    for suffix in (".lock.yml", ".yml", ".md"):
        if name.endswith(suffix):
            name = name[: -len(suffix)]
            break
    if name not in LANE_FORMS:
        raise ValueError(f"workflow {workflow_ref!r} is not a maintenance lane")
    return name


def prose_lines(body):
    """Yield (line, in_fence) for each body line."""
    in_fence = False
    for line in body.splitlines():
        if FENCE.match(line):
            in_fence = not in_fence
            yield line, True
            continue
        yield line, in_fence


def heading_errors(body, labels):
    headings = []
    sections = []
    leading = []
    for line, in_fence in prose_lines(body):
        match = None if in_fence else HEADING.match(line)
        if match:
            headings.append((len(match.group(1)), match.group(2).strip()))
            sections.append([])
        elif sections:
            sections[-1].append(line)
        else:
            leading.append(line)
    expected = [(3, label) for label in labels]
    if headings != expected:
        found = ", ".join(f"{'#' * level} {text}" for level, text in headings) or "none"
        return [f"headings must be exactly {', '.join('### ' + label for label in labels)} in order; found {found}"]
    errors = []
    if any(line.strip() for line in leading):
        errors.append("body must start with the first form heading")
    for label, section in zip(labels, sections):
        if not any(line.strip() for line in section):
            errors.append(f"section '{label}' is empty")
    return errors


def line_count(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if not data:
        return 0
    return data.count(b"\n") + (0 if data.endswith(b"\n") else 1)


def cited_paths(text, root):
    """Return (citations, text_without_valid_citations, errors)."""
    root = Path(root)
    top_level = {entry.name for entry in root.iterdir()}
    citations = []
    errors = []

    def check(match):
        token = match.group("path")
        path = token.rstrip(".")
        if path.startswith("./"):
            path = path[2:]
        has_extension = re.search(r"\.(?:" + FILE_EXTENSIONS + r")$", path) is not None
        first = path.split("/", 1)[0]
        if not (has_extension or first in top_level or match.group(0).startswith(".")):
            return match.group(0)
        start = match.group("start") or match.group("hstart")
        end = match.group("end") or match.group("hend") or start
        citations.append(path)
        target = (root / path).resolve()
        if root.resolve() not in target.parents and target != root.resolve():
            errors.append(f"{path} is outside the repository")
            return match.group(0)
        if not target.exists():
            errors.append(f"{path} does not exist")
            return match.group(0)
        if start:
            if not target.is_file():
                errors.append(f"{path} is not a file but has a line range")
                return match.group(0)
            first_line, last_line = int(start), int(end)
            total = line_count(target)
            if first_line < 1 or last_line < first_line or last_line > total:
                errors.append(f"{path}:{first_line}-{last_line} is outside the file's {total} lines")
                return match.group(0)
        return " "

    urls = {}

    def mask(match):
        key = f"\x00{len(urls)}\x00"
        urls[key] = match.group(0)
        return key

    remaining = PATH_TOKEN.sub(check, URL.sub(mask, text))
    for key, url in urls.items():
        remaining = remaining.replace(key, url)
    return citations, remaining, errors


def process_term_errors(text):
    errors = []
    for pattern, name in PROCESS_TERMS:
        match = pattern.search(text)
        if match:
            errors.append(f"mentions {name} ({match.group(0)!r})")
    return errors


def normalize_title(title, prefix):
    title = title.strip()
    if prefix and title.lower().startswith(prefix.strip().lower()):
        title = title[len(prefix.strip()):]
    return re.sub(r"\s+", " ", title).strip().rstrip(".").casefold()


def duplicate_errors(title, prefix, existing_titles):
    proposed = normalize_title(title, prefix)
    for existing in existing_titles:
        if normalize_title(existing, prefix) == proposed:
            return [f"title duplicates existing issue {existing!r}"]
    return []


def get_json(url, headers, attempts=3):
    """Return the decoded JSON body and Link header of a GET request, retrying transient failures."""
    for attempt in range(attempts):
        try:
            request = urllib.request.Request(url, headers=headers)
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.loads(response.read()), response.headers.get("Link", "")
        except urllib.error.HTTPError:
            raise
        except (urllib.error.URLError, http.client.HTTPException, OSError):
            if attempt == attempts - 1:
                raise


def fetch_issue_titles(repository, token):
    """Return the titles of every open and closed issue, excluding pull requests."""
    headers = {"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28"}
    attempts = [dict(headers, Authorization=f"Bearer {token}")] if token else []
    attempts.append(headers)
    last_error = None
    for attempt in attempts:
        titles = []
        url = f"https://api.github.com/repos/{repository}/issues?state=all&per_page=100"
        try:
            while url:
                page, links = get_json(url, attempt)
                titles.extend(item["title"] for item in page if "pull_request" not in item)
                next_link = re.search(r"<([^>]+)>;\s*rel=\"next\"", links)
                url = next_link.group(1) if next_link else None
            return titles
        except (urllib.error.URLError, http.client.HTTPException, OSError, ValueError, KeyError, TypeError) as error:
            last_error = error
    raise RuntimeError(f"cannot list existing issues of {repository}: {last_error}")


def proposed_issues(agent_output):
    data = json.loads(Path(agent_output).read_text(encoding="utf-8"))
    items = data.get("items")
    if not isinstance(items, list):
        raise ValueError(f"{agent_output} has no items list")
    return [item for item in items if item.get("type") == "create_issue"]


ISSUE_FIELDS = {"type", "title", "body"}


def field_errors(issue):
    """Reject fields beyond title and body, such as labels or a parent issue, which would be published unchecked."""
    extra = sorted(key for key, value in issue.items() if key not in ISSUE_FIELDS and value not in (None, "", [], {}))
    return [f"issue sets {', '.join(extra)}; only title and body may be proposed"] if extra else []


def issue_errors(issue, *, labels, prefix, root, existing_titles):
    title = issue.get("title") or ""
    body = issue.get("body") or ""
    errors = field_errors(issue)
    errors.extend(heading_errors(body, labels))
    _, remaining, path_errors = cited_paths(f"{title}\n{body}", root)
    errors.extend(path_errors)
    errors.extend(process_term_errors(remaining))
    errors.extend(duplicate_errors(title, prefix, existing_titles))
    return errors


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--agent-output", required=True)
    parser.add_argument("--workflow-ref", default=os.environ.get("GITHUB_WORKFLOW_REF", ""))
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY", ""))
    parser.add_argument("--root", default=str(ROOT))
    args = parser.parse_args(argv)

    issues = proposed_issues(args.agent_output)
    if not issues:
        print("No issue proposed.")
        return 0
    if len(issues) > 1:
        print(f"::error::{len(issues)} issues proposed; a maintenance run proposes at most one")
        return 1

    root = Path(args.root)
    lane = lane_from_workflow_ref(args.workflow_ref)
    labels = form_labels(root / LANE_FORMS[lane])
    prefix = title_prefix(root / ".github" / "workflows" / f"{lane}.md")
    existing = fetch_issue_titles(args.repository, os.environ.get("GH_TOKEN", ""))

    errors = issue_errors(issues[0], labels=labels, prefix=prefix, root=root, existing_titles=existing)
    for error in errors:
        print(f"::error::{error}")
    if errors:
        return 1
    print(f"Proposed issue follows {LANE_FORMS[lane]} and cites only existing repository content.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
