import json

from check_command_motion_backend import equation_sample
from check_schemas import ROOT, load_json, validator_for


PROFILES = ("frost-body", "frost-depth", "elastomer-body", "elastomer-depth", "toggle-travel")
GRID = 0.0001
RETARGET_TIMES = (0.016, 0.033, 0.05, 0.075, 0.1, 0.15)


def measure(dynamics):
    validator_for("schemas/spring-dynamics.schema.json").validate(dynamics)
    channel = {"dynamics": dynamics, "initial": {"position": 0, "velocity": 0}}
    samples = [equation_sample(channel, 1, index * GRID, False) for index in range(10001)]
    ninety = next((index * GRID for index, sample in enumerate(samples)
                   if sample["state"]["position"] >= 0.9), None)
    settled = next((index * GRID for index, sample in enumerate(samples) if sample["settled"]), None)
    if ninety is None or settled is None:
        raise ValueError("unit step did not respond and settle within the one-second measurement window")
    retargets = []
    for elapsed in RETARGET_TIMES:
        retained = equation_sample(channel, 1, elapsed, False)["state"]
        release = {"dynamics": dynamics, "initial": retained}
        at_zero = equation_sample(release, 0, 0, False)
        if at_zero["state"] != retained:
            raise ValueError("retarget does not preserve raw state at time zero")
        terminal = next((index * GRID for index in range(10001)
                         if equation_sample(release, 0, index * GRID, False)["settled"]), None)
        if terminal is None:
            raise ValueError("retarget did not settle within the one-second measurement window")
        retargets.append({"interruptionSeconds": elapsed, "retainedState": retained,
                          "settledSecondsFromRestart": terminal})
    return {"measurementGridSeconds": GRID, "unitStep90Seconds": ninety,
            "unitStepSettledSeconds": settled,
            "peakPosition": max(sample["state"]["position"] for sample in samples),
            "monotoneUnitStep": all(after["state"]["position"] >= before["state"]["position"]
                                    for before, after in zip(samples, samples[1:])),
            "retargets": retargets}


def main():
    reports = {name: measure(load_json(ROOT / f"definitions/motion/{name}.json")) for name in PROFILES}
    print(json.dumps(reports, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
