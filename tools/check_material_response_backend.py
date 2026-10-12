from check_color_guard_backend import check_backend
from check_spring_trajectory_backend import trajectory_mismatch


if __name__ == "__main__":
    raise SystemExit(check_backend(
        "authored material response", "schemas/spring-trajectory-case.schema.json",
        "schemas/spring-trajectory-request.schema.json", "schemas/spring-trajectory-result.schema.json",
        "conformance/motion/material-response-cases.json", compare=trajectory_mismatch,
    ))
