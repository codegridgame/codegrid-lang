"""Verify direct workspace-crate dependencies against the repository contract."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


EXPECTED_DEPENDENCIES = {
    "codegrid-model": set(),
    "codegrid-syntax": {"codegrid-model"},
    "codegrid-hir": {"codegrid-model", "codegrid-syntax"},
    "codegrid-ir": {"codegrid-model"},
    "codegrid-compiler": {
        "codegrid-hir",
        "codegrid-ir",
        "codegrid-model",
        "codegrid-syntax",
    },
    "codegrid-vm": {"codegrid-ir", "codegrid-model"},
    "codegrid-runtime-api": {
        "codegrid-compiler",
        "codegrid-ir",
        "codegrid-model",
        "codegrid-vm",
    },
    "codegrid-cli": {
        "codegrid-compiler",
        "codegrid-ir",
        "codegrid-model",
        "codegrid-syntax",
        "codegrid-vm",
    },
    "codegrid-lsp": {
        "codegrid-compiler",
        "codegrid-model",
        "codegrid-syntax",
    },
    "codegrid-wasm-browser": {"codegrid-runtime-api"},
    "codegrid-wasm-server": {"codegrid-runtime-api"},
}


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    metadata_json = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    metadata = json.loads(metadata_json)
    workspace_member_ids = set(metadata["workspace_members"])
    packages = {
        package["name"]: package
        for package in metadata["packages"]
        if package["id"] in workspace_member_ids
    }

    errors = []
    actual_package_names = set(packages)
    for name in sorted(actual_package_names - set(EXPECTED_DEPENDENCIES)):
        errors.append(f"unexpected workspace package: {name}")
    for name in sorted(set(EXPECTED_DEPENDENCIES) - actual_package_names):
        errors.append(f"missing workspace package: {name}")

    required_agent_files = [
        root / "AGENTS.md",
        root / "editors" / "vscode" / "AGENTS.md",
    ]
    required_agent_files.extend(
        Path(package["manifest_path"]).parent / "AGENTS.md"
        for package in packages.values()
    )
    for path in required_agent_files:
        if not path.is_file():
            errors.append(f"missing module rules: {path.relative_to(root)}")

    for name in sorted(actual_package_names & set(EXPECTED_DEPENDENCIES)):
        dependencies = {
            dependency["name"]
            for dependency in packages[name]["dependencies"]
            if dependency.get("path")
            and dependency["name"] in actual_package_names
        }
        expected = EXPECTED_DEPENDENCIES[name]
        for dependency in sorted(dependencies - expected):
            errors.append(f"{name} has forbidden workspace dependency {dependency}")
        for dependency in sorted(expected - dependencies):
            errors.append(f"{name} is missing workspace dependency {dependency}")

    if errors:
        print(
            "Workspace dependency graph does not match the architecture:",
            file=sys.stderr,
        )
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("Workspace dependency graph matches the enforced dependency contract.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
