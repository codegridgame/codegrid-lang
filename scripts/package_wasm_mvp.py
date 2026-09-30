#!/usr/bin/env python3
"""Package already-built CodeGrid MVP WebAssembly artifacts deterministically."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
import uuid
import zipfile
from pathlib import Path
from typing import Iterable


PAGE_SIZE = 65_536
WASM32_MAX_MEMORY_BYTES = 4_294_967_296
ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)


class PackageError(Exception):
    """An expected packaging or input validation failure."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Package previously built CodeGrid MVP WebAssembly artifacts. "
            "This command does not build or publish artifacts."
        )
    )
    parser.add_argument(
        "--version",
        required=True,
        help="release version; must match both WASM crate Cargo.toml files",
    )
    parser.add_argument(
        "--output-dir",
        default="dist",
        help="directory for the staged package and ZIP archive (default: dist)",
    )
    return parser.parse_args()


def package_version(cargo_toml: Path) -> str:
    try:
        with cargo_toml.open("rb") as file:
            data = tomllib.load(file)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise PackageError(f"cannot read {cargo_toml}: {error}") from error

    try:
        value = data["package"]["version"]
    except (KeyError, TypeError) as error:
        raise PackageError(f"{cargo_toml} does not define package.version") from error
    if not isinstance(value, str):
        raise PackageError(f"{cargo_toml} package.version must be a string")
    return value


def validate_release_version(root: Path, requested: str) -> str:
    if not requested or not re.fullmatch(r"[0-9A-Za-z.+-]+", requested):
        raise PackageError("--version must be a non-empty version string without path characters")

    package_files = (
        root / "crates/codegrid-wasm-browser/Cargo.toml",
        root / "crates/codegrid-wasm-server/Cargo.toml",
    )
    versions = {path: package_version(path) for path in package_files}
    mismatches = [f"{path}: {version}" for path, version in versions.items() if version != requested]
    if mismatches:
        details = "; ".join(mismatches)
        raise PackageError(
            f"--version {requested!r} must match both WASM crate versions; found {details}"
        )
    return requested


def configured_memory_ceiling() -> int:
    raw = os.environ.get("CODEGRID_WASM_MAX_MEMORY_BYTES")
    if raw is None:
        raise PackageError("CODEGRID_WASM_MAX_MEMORY_BYTES must be set for packaging")
    if not raw or not raw.isascii() or not raw.isdecimal():
        raise PackageError("CODEGRID_WASM_MAX_MEMORY_BYTES must be an unsigned decimal byte count")
    memory_bytes = int(raw, 10)
    if memory_bytes <= 0 or memory_bytes % PAGE_SIZE != 0 or memory_bytes > WASM32_MAX_MEMORY_BYTES:
        raise PackageError(
            "CODEGRID_WASM_MAX_MEMORY_BYTES must be a positive multiple of 65536 "
            "no greater than 4294967296"
        )
    return memory_bytes


def wasm_bindgen_version() -> str:
    try:
        result = subprocess.run(
            ["wasm-bindgen", "--version"],
            check=False,
            capture_output=True,
            text=True,
            timeout=15,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise PackageError(f"cannot determine wasm-bindgen CLI version: {error}") from error

    output = result.stdout.strip()
    if result.returncode != 0:
        detail = result.stderr.strip() or output or f"exit status {result.returncode}"
        raise PackageError(f"wasm-bindgen --version failed: {detail}")
    match = re.fullmatch(r"wasm-bindgen\s+([^\s]+)", output)
    if match is None:
        raise PackageError(f"unexpected wasm-bindgen version output: {output!r}")
    return match.group(1)


def ensure_regular_file(path: Path) -> None:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise PackageError(f"required file is missing or unreadable: {path}: {error}") from error
    if not stat.S_ISREG(metadata.st_mode):
        raise PackageError(f"expected a regular file: {path}")


def collect_tree_files(source_root: Path) -> list[tuple[Path, str]]:
    if not source_root.is_dir() or source_root.is_symlink():
        raise PackageError(f"required bindings directory is missing or invalid: {source_root}")

    files: list[tuple[Path, str]] = []
    for current, directory_names, file_names in os.walk(source_root, followlinks=False):
        current_path = Path(current)
        directory_names.sort()
        file_names.sort()
        for name in directory_names:
            path = current_path / name
            if path.is_symlink():
                raise PackageError(f"symbolic links are not allowed in bindings: {path}")
            if not path.is_dir():
                raise PackageError(f"unexpected non-directory binding entry: {path}")
        for name in file_names:
            path = current_path / name
            if path.is_symlink() or not path.is_file():
                raise PackageError(f"expected a regular binding file: {path}")
            relative = path.relative_to(source_root).as_posix()
            files.append((path, relative))

    if not files:
        raise PackageError(f"bindings directory contains no files: {source_root}")
    return files


def copy_payload_file(source: Path, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    with source.open("rb") as source_file, destination.open("wb") as destination_file:
        shutil.copyfileobj(source_file, destination_file)


def read_u32_leb(data: bytes, offset: int, limit: int) -> tuple[int, int]:
    value = 0
    shift = 0
    for _ in range(5):
        if offset >= limit:
            raise PackageError("unexpected end of WebAssembly section while reading an integer")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            if value > 0xFFFFFFFF:
                raise PackageError("WebAssembly section integer exceeds the u32 range")
            return value, offset
        shift += 7
    raise PackageError("WebAssembly section integer uses more than five bytes")


def wasm_memory_maximum(path: Path) -> int:
    try:
        data = path.read_bytes()
    except OSError as error:
        raise PackageError(f"cannot read WebAssembly module {path}: {error}") from error
    if len(data) < 8 or data[:8] != b"\x00asm\x01\x00\x00\x00":
        raise PackageError(f"not a WebAssembly 1.0 module: {path}")

    offset = 8
    memory_maximum: int | None = None
    while offset < len(data):
        section_id = data[offset]
        offset += 1
        section_size, payload_start = read_u32_leb(data, offset, len(data))
        section_end = payload_start + section_size
        if section_end > len(data):
            raise PackageError(f"truncated WebAssembly section in {path}")
        if section_id == 5:
            if memory_maximum is not None:
                raise PackageError(f"multiple WebAssembly memory sections in {path}")
            count, cursor = read_u32_leb(data, payload_start, section_end)
            if count != 1:
                raise PackageError(f"expected one defined WebAssembly memory in {path}, found {count}")
            flags, cursor = read_u32_leb(data, cursor, section_end)
            if flags & 1 == 0:
                raise PackageError(f"WebAssembly memory has no declared maximum in {path}")
            if flags & 4:
                raise PackageError(f"WebAssembly memory64 is unsupported for this wasm32 package: {path}")
            if flags & ~0x3:
                raise PackageError(f"unsupported WebAssembly memory flags in {path}")
            minimum_pages, cursor = read_u32_leb(data, cursor, section_end)
            maximum_pages, cursor = read_u32_leb(data, cursor, section_end)
            if cursor != section_end or minimum_pages > maximum_pages:
                raise PackageError(f"invalid WebAssembly memory limits in {path}")
            memory_maximum = maximum_pages * PAGE_SIZE
        offset = section_end

    if memory_maximum is None:
        raise PackageError(f"WebAssembly module has no defined memory section: {path}")
    return memory_maximum


def require_memory_maximum(path: Path, expected_bytes: int) -> None:
    actual_bytes = wasm_memory_maximum(path)
    if actual_bytes != expected_bytes:
        raise PackageError(
            f"{path} declares a {actual_bytes}-byte linear-memory maximum; "
            f"expected {expected_bytes} bytes from CODEGRID_WASM_MAX_MEMORY_BYTES"
        )


def write_payload(root: Path, stage: Path, version: str, memory_bytes: int) -> list[Path]:
    browser_web = root / "target/browser-bindings"
    browser_node = root / "target/browser-node-bindings"
    server_wasm = root / "target/wasm32-unknown-unknown/release/codegrid_wasm_server.wasm"
    license_file = root / "LICENSE"

    ensure_regular_file(server_wasm)
    ensure_regular_file(license_file)
    require_memory_maximum(server_wasm, memory_bytes)

    payload_paths: list[Path] = []
    copy_payload_file(license_file, stage / "LICENSE")
    payload_paths.append(Path("LICENSE"))

    distribution_doc = root / "docs/wasm-distribution.md"
    ensure_regular_file(distribution_doc)
    readme = distribution_doc.read_text(encoding="utf-8")
    local_contract_link = "../crates/codegrid-wasm-server/README.md"
    if readme.count(local_contract_link) != 1:
        raise PackageError(
            "docs/wasm-distribution.md must contain exactly one server contract link "
            "for versioned package rewriting"
        )
    versioned_contract_link = (
        "https://github.com/codegridgame/codegrid-lang/blob/"
        f"wasm-mvp-v{version}/crates/codegrid-wasm-server/README.md"
    )
    (stage / "README.md").write_text(
        readme.replace(local_contract_link, versioned_contract_link),
        encoding="utf-8",
        newline="\n",
    )
    payload_paths.append(Path("README.md"))

    for source_root, destination_root in (
        (browser_web, stage / "browser/web"),
        (browser_node, stage / "browser/nodejs"),
    ):
        source_files = collect_tree_files(source_root)
        wasm_files = [path for path, _ in source_files if path.suffix.lower() == ".wasm"]
        if len(wasm_files) != 1:
            raise PackageError(
                f"expected one generated browser WASM module in {source_root}, found {len(wasm_files)}"
            )
        require_memory_maximum(wasm_files[0], memory_bytes)
        for source_file, relative in source_files:
            destination = destination_root / Path(relative)
            copy_payload_file(source_file, destination)
            payload_paths.append(destination.relative_to(stage))

    copy_payload_file(server_wasm, stage / "server/codegrid_wasm_server.wasm")
    payload_paths.append(Path("server/codegrid_wasm_server.wasm"))
    return sorted(payload_paths, key=lambda path: path.as_posix())


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_manifest(
    stage: Path,
    version: str,
    memory_bytes: int,
    bindgen_version: str,
    payload_paths: Iterable[Path],
) -> None:
    hashes = {
        path.as_posix(): sha256_file(stage / path)
        for path in sorted(payload_paths, key=lambda item: item.as_posix())
    }
    manifest = {
        "format": "codegrid-wasm-mvp-package-v1",
        "version": version,
        "source_profile": "MVP 0.1",
        "runtime_api_version": 2,
        "server_abi_version": 3,
        "wasm32_linear_memory_ceiling_bytes": memory_bytes,
        "wasm_bindgen_version": bindgen_version,
        "payload_sha256": hashes,
    }
    content = json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    (stage / "manifest.json").write_text(content, encoding="utf-8", newline="\n")


def zip_package(stage: Path, destination: Path) -> None:
    members = sorted(
        (path for path in stage.rglob("*") if path.is_file()),
        key=lambda path: path.relative_to(stage).as_posix(),
    )
    with zipfile.ZipFile(destination, mode="w", compression=zipfile.ZIP_STORED) as archive:
        for path in members:
            member_name = path.relative_to(stage).as_posix()
            info = zipfile.ZipInfo(member_name, date_time=ZIP_TIMESTAMP)
            info.compress_type = zipfile.ZIP_STORED
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o644) << 16
            info.internal_attr = 0
            info.extra = b""
            info.comment = b""
            archive.writestr(info, path.read_bytes())


def validate_output_targets(stage_target: Path, archive_target: Path) -> None:
    if stage_target.is_symlink() or (stage_target.exists() and not stage_target.is_dir()):
        raise PackageError(f"staging target must be a directory, not a file or symlink: {stage_target}")
    if archive_target.is_symlink() or (archive_target.exists() and not archive_target.is_file()):
        raise PackageError(f"archive target must be a regular file, not a directory or symlink: {archive_target}")


def publish_local_outputs(staged: Path, temporary_zip: Path, stage_target: Path, archive_target: Path) -> None:
    backup: Path | None = None
    if stage_target.exists():
        backup = stage_target.with_name(f".{stage_target.name}.backup-{uuid.uuid4().hex}")
        os.replace(stage_target, backup)
    try:
        os.replace(staged, stage_target)
        os.replace(temporary_zip, archive_target)
    except OSError:
        if stage_target.exists():
            shutil.rmtree(stage_target)
        if backup is not None and backup.exists():
            os.replace(backup, stage_target)
        raise
    if backup is not None:
        shutil.rmtree(backup)


def build_package(root: Path, version: str, output_dir: Path) -> tuple[Path, Path, int]:
    selected_version = validate_release_version(root, version)
    memory_bytes = configured_memory_ceiling()
    bindgen_version = wasm_bindgen_version()

    output_dir.mkdir(parents=True, exist_ok=True)
    package_name = f"wasm-mvp-{selected_version}"
    stage_target = output_dir / package_name
    archive_target = output_dir / f"{package_name}.zip"
    validate_output_targets(stage_target, archive_target)

    with tempfile.TemporaryDirectory(prefix=f".{package_name}-", dir=output_dir) as temporary_name:
        temporary_root = Path(temporary_name)
        staged = temporary_root / package_name
        staged.mkdir()
        payload_paths = write_payload(root, staged, selected_version, memory_bytes)
        write_manifest(staged, selected_version, memory_bytes, bindgen_version, payload_paths)
        temporary_zip = temporary_root / f"{package_name}.zip"
        zip_package(staged, temporary_zip)
        publish_local_outputs(staged, temporary_zip, stage_target, archive_target)

    return stage_target, archive_target, len(payload_paths)


def main() -> int:
    arguments = parse_args()
    root = Path(__file__).resolve().parents[1]
    output_dir = Path(arguments.output_dir).expanduser().resolve()
    try:
        stage, archive, payload_count = build_package(root, arguments.version, output_dir)
    except PackageError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    except OSError as error:
        print(f"error: packaging failed: {error}", file=sys.stderr)
        return 2

    print(f"Packaged {payload_count} payload files for CodeGrid MVP {arguments.version}.")
    print(f"Staged package: {stage}")
    print(f"ZIP archive: {archive}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
