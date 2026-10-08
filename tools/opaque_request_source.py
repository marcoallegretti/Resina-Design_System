import copy
import json

from check_schemas import replace_at_pointer


_MEMBERS = (
    "schemaVersion", "theme", "surface", "size", "appearance", "foregroundRole",
    "postTreatmentBackdrop", "adjacentColor", "minimumContentContrast", "minimumEdgeContrast",
)


def opaque_request_failures(document, pointer):
    body = document
    for member in pointer.strip("/").split("/") if pointer else ():
        body = body[member]
    tagged = copy.deepcopy(body)
    tagged["foregroundRole"] = {body["foregroundRole"]: None}
    failures = []
    for name, value in (
        ("positional opaque request", [body[member] for member in _MEMBERS]),
        ("object-encoded foreground role", tagged),
    ):
        request = copy.deepcopy(document)
        if pointer:
            replace_at_pointer(request, pointer, value)
        else:
            request = value
        failures.append((name, json.dumps(request, ensure_ascii=False, allow_nan=False)))
    return tuple(failures)
