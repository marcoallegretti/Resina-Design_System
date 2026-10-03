import copy
import json

from check_schemas import pointer_member, replace_at_pointer


def duplicate_member_source(document, pointer):
    return _member_source(document, pointer, duplicate=True)


def nonfinite_member_source(document, pointer):
    return _member_source(document, pointer, duplicate=False)


def _member_source(document, pointer, *, duplicate):
    options = {"ensure_ascii": False, "allow_nan": False, "separators": (",", ":")}
    source = json.dumps(document, **options)
    marker = "source-member-value"
    while json.dumps(marker, **options) in source:
        marker += "-"
    changed = copy.deepcopy(document)
    replace_at_pointer(changed, pointer, marker)
    segments = [part.replace("~1", "/").replace("~0", "~") for part in pointer[1:].split("/")]
    parent = document
    for segment in segments[:-1]:
        parent = parent[pointer_member(parent, segment, pointer)]
    if not isinstance(parent, dict):
        raise ValueError(f"source target must be an object member: {pointer!r}")
    member = pointer_member(parent, segments[-1], pointer)
    original = parent[member]
    if duplicate:
        value = json.dumps(original, **options)
        replacement = value + "," + json.dumps(member, **options) + ":" + value
    else:
        if isinstance(original, bool) or not isinstance(original, (int, float)):
            raise ValueError(f"nonfinite target must be a number: {pointer!r}")
        replacement = "1e400"
    return json.dumps(changed, **options).replace(json.dumps(marker, **options), replacement, 1)
