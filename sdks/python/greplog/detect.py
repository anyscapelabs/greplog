"""Framework detection and greplog.config.json emission."""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Optional


@dataclass
class DetectionResult:
    service_name: str
    language: str
    framework: Optional[str]
    project_file: Optional[str]


def _installed_packages() -> set[str]:
    try:
        from importlib.metadata import distributions

        names: set[str] = set()
        for dist in distributions():
            name = dist.metadata.get("Name")
            if name:
                names.add(name.lower())
        return names
    except Exception:
        return set()


def detect_service_name_from_project() -> str:
    from greplog.internal import detect_service_name

    return detect_service_name()


def detect_framework(workspace: Optional[str] = None) -> DetectionResult:
    _ = workspace
    result = DetectionResult(
        service_name=detect_service_name_from_project(),
        language="python",
        framework=None,
        project_file="pyproject.toml" if os.path.exists("pyproject.toml") else None,
    )

    installed = _installed_packages()
    if "fastapi" in installed:
        result.framework = "fastapi"
    elif "flask" in installed:
        result.framework = "flask"
    elif "django" in installed:
        result.framework = "django"

    return result


def write_config(detection: DetectionResult, out_path: Optional[str] = None) -> None:
    config_path = out_path or os.path.join(os.getcwd(), "greplog.config.json")
    config = {
        "service_name": detection.service_name,
        "language": detection.language,
        "framework": detection.framework,
        "detected_at": datetime.now(timezone.utc).isoformat(),
    }
    try:
        with open(config_path, "w", encoding="utf-8") as fh:
            json.dump(config, fh, indent=2)
    except Exception:
        pass
