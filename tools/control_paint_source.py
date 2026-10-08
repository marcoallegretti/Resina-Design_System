import json


def positional_control_request(document, members):
    return json.dumps([document[member] for member in members], ensure_ascii=False, allow_nan=False)
