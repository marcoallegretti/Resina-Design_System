import copy
import json

from check_schemas import replace_at_pointer


def positional_paint_request(document, pointer):
    paint = document
    for member in pointer.strip("/").split("/") if pointer else ():
        paint = paint[member]
    positional = [paint["schemaVersion"], paint["body"], paint["surroundingColor"]]
    request = copy.deepcopy(document)
    if pointer:
        replace_at_pointer(request, pointer, positional)
    else:
        request = positional
    return json.dumps(request, ensure_ascii=False, allow_nan=False)
