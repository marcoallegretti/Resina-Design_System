import json
import math
import random
import sys
from pathlib import Path
from decimal import Context, Decimal, localcontext

CONTEXT = Context(prec=800, Emax=999999999, Emin=-999999999)
MAXIMUM = Decimal(sys.float_info.max)
SMALLEST = Decimal(5e-324)


class RangeError(Exception):
    pass


def representable(value):
    return abs(value) <= MAXIMUM


def nonzero_representable(value):
    return representable(value) and abs(value) >= SMALLEST / 2


def hypot(first, second):
    return (first * first + second * second).sqrt()


def trajectory(dynamics, initial, target, time, jitter=0):
    with localcontext(CONTEXT):
        return _trajectory(dynamics, initial, target, time, Decimal(jitter))


def _trajectory(dynamics, initial, target, time, jitter):
    mass, stiffness, damping = (Decimal(dynamics[name]) for name in ("mass", "stiffness", "damping"))
    position_limit = Decimal(dynamics["positionThreshold"])
    velocity_limit = Decimal(dynamics["velocityThreshold"])
    start, speed = Decimal(initial["position"]), Decimal(initial["velocity"])
    target, time = Decimal(target), Decimal(time)
    root_mass, root_stiffness = mass.sqrt(), stiffness.sqrt()
    omega = root_stiffness / root_mass
    scale = root_mass * root_stiffness
    for name, value in (("natural frequency", omega), ("mass-stiffness scale", scale)):
        if not nonzero_representable(value):
            raise RangeError(name)
    zeta = damping / (2 * scale)
    if not representable(zeta) or (damping > 0 and not nonzero_representable(zeta)):
        raise RangeError("damping ratio")
    q = speed / omega
    if not representable(q) or (speed != 0 and not nonzero_representable(q)):
        raise RangeError("normalized velocity")
    u = omega * time
    if not representable(u) or (time > 0 and not nonzero_representable(u)):
        raise RangeError("normalized time")
    y0 = start - target
    if not representable(y0):
        raise RangeError("initial displacement")
    if u == 0:
        y, w = y0, q
    elif zeta < 1:
        beta = (1 - zeta * zeta).sqrt()
        phase = beta * u
        angle = float(phase * (1 + jitter))
        cosine, sine = Decimal(math.cos(angle)), Decimal(math.sin(angle))
        kernel = sine / beta if phase != 0 else u
        decay = (-zeta * u).exp()
        y = decay * (y0 * cosine + (q + zeta * y0) * kernel)
        w = decay * (q * cosine - (y0 + zeta * q) * kernel)
    elif zeta == 1:
        decay = (-u).exp()
        y = decay * (y0 + (q + y0) * u)
        w = decay * (q - (q + y0) * u)
    else:
        root = (zeta * zeta - 1).sqrt()
        slow = -1 / (zeta + root)
        fast = -zeta - root
        a = (q - fast * y0) / (slow - fast)
        b = y0 - a
        y = a * (slow * u).exp() + b * (fast * u).exp()
        w = slow * a * (slow * u).exp() + fast * b * (fast * u).exp()
    values = (y, w, omega * y, omega * w, hypot(y, w), omega * hypot(y, w))
    if not all(representable(value) for value in values):
        raise RangeError("solution")
    settled = values[4] <= position_limit and values[5] <= velocity_limit
    if settled:
        return target, Decimal(0), True
    position = start if time == 0 else target + y
    if not representable(position):
        raise RangeError("position")
    return position, omega * w, False


def hostile_requests(seed, count):
    generator = random.Random(seed)

    def magnitude(low, high):
        return 10.0 ** generator.uniform(low, high)

    def signed(low, high):
        return generator.choice((-1, 1)) * magnitude(low, high)

    regimes = (
        {"mass": (-1, 1), "stiffness": (0, 4), "damping": (-1, 3), "position": (-3, 3), "velocity": (-3, 3), "time": (-3, 1)},
        {"mass": (-300, -290), "stiffness": (-300, -290), "damping": (-300, 300), "position": (-5, 300), "velocity": (-5, 300), "time": (-5, 305)},
        {"mass": (-3, 3), "stiffness": (-3, 5), "damping": (-3, 5), "position": (200, 308), "velocity": (100, 308), "time": (100, 306)},
        {"mass": (-5, 5), "stiffness": (-5, 5), "damping": (-5, 5), "position": (-300, 300), "velocity": (-300, 300), "time": (-300, 307)},
    )
    requests = []
    for index in range(count):
        regime = regimes[index % len(regimes)]
        requests.append({
            "schemaVersion": "0.1.0",
            "dynamics": {
                "schemaVersion": "0.1.0",
                "mass": magnitude(*regime["mass"]),
                "stiffness": magnitude(*regime["stiffness"]),
                "damping": generator.choice((0.0, magnitude(*regime["damping"]))),
                "positionThreshold": magnitude(-8, -2),
                "velocityThreshold": magnitude(-8, -2),
            },
            "initial": {"position": signed(*regime["position"]), "velocity": signed(*regime["velocity"])},
            "target": generator.choice((0.0, signed(-3, 3), signed(*regime["position"]))),
            "time": generator.choice((0.0, magnitude(*regime["time"]))),
            "reducedMotion": False,
        })
    return requests


def verdict(request):
    try:
        position, velocity, settled = trajectory(
            request["dynamics"], request["initial"], request["target"], request["time"])
    except RangeError as error:
        return None, str(error)
    return {
        "schemaVersion": "0.1.0",
        "representation": "spring",
        "target": request["target"],
        "state": {"position": float(position), "velocity": float(velocity)},
        "settled": settled,
    }, None


def well_conditioned(request):
    try:
        exact = trajectory(request["dynamics"], request["initial"], request["target"], request["time"])
        shifted = trajectory(request["dynamics"], request["initial"], request["target"], request["time"], 4e-16)
    except RangeError:
        return True
    if exact[2] != shifted[2]:
        return False
    with localcontext(CONTEXT):
        offset = exact[0] - Decimal(request["target"])
        if not exact[2] and abs(exact[0]) < Decimal("1e-6") * abs(offset):
            return False
    return all(
        abs(a - b) <= Decimal("1e-10") * max(abs(a), Decimal(1e-300))
        for a, b in zip(exact[:2], shifted[:2])
    )


SEED = 20261023
RANDOM_CASES = 160
CASES = Path(__file__).resolve().parent.parent / "conformance/motion/spring-range-cases.json"


def named_request(position, velocity, time, mass, stiffness, damping, target=0.0):
    return {
        "schemaVersion": "0.1.0",
        "dynamics": {
            "schemaVersion": "0.1.0", "mass": mass, "stiffness": stiffness, "damping": damping,
            "positionThreshold": 1e-6, "velocityThreshold": 1e-6,
        },
        "initial": {"position": position, "velocity": velocity},
        "target": target, "time": time, "reducedMotion": False,
    }


def case_for(name, request):
    result, reason = verdict(request)
    case = {"name": name, "request": request, "requestSchemaValid": True}
    if result is None:
        case["errorContains"] = reason
    else:
        case["expected"] = result
    return case


def cases():
    named = [
        ("a 1e308 displacement decays to the target before its energy is bounded",
         named_request(1e308, 0.0, 100.0, 1.0, 1600.0, 80.0)),
        ("a 1e308 displacement far in time settles", named_request(1e308, 0.0, 1e5, 1.0, 1600.0, 80.0)),
        ("a damped 1e307 displacement with 1e153 velocity at 1e200 seconds settles",
         named_request(1e307, 1e153, 1e200, 0.5, 1e4, 50.0)),
        ("an undamped 1e307 displacement with 1e153 velocity has no representable solution",
         named_request(1e307, 1e153, 1e200, 0.5, 1e4, 0.0)),
        ("a 1e308 displacement at time zero has no representable scaled displacement",
         named_request(1e308, 0.0, 0.0, 1.0, 1600.0, 80.0)),
        ("a normalized time beyond binary64 is rejected", named_request(1.0, 0.0, 1e307, 1.0, 1600.0, 80.0)),
        ("extreme mass and stiffness with a representable ratio stay well defined",
         named_request(1.0, 0.0, 1e300, 1e-300, 1e-300, 1e-300)),
    ]
    result = [case_for(name, request) for name, request in named]
    for index, request in enumerate(hostile_requests(SEED, RANDOM_CASES * 2)):
        if len(result) >= len(named) + RANDOM_CASES:
            break
        if well_conditioned(request):
            result.append(case_for(f"seeded hostile request {index}", request))
    return result


if __name__ == "__main__":
    CASES.write_text(json.dumps(cases(), indent=2) + chr(10), encoding="utf-8", newline=chr(10))
