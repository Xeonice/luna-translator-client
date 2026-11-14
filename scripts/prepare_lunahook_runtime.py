#!/usr/bin/env python3
"""Extract only the pinned LunaHook runtime; never execute or inject anything."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import stat
import zipfile


DEFAULT_LOCK = Path(__file__).resolve().parents[1] / "native/upstream.lock.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def prepare(archive, output, lock_path=DEFAULT_LOCK):
    archive, output, lock_path = Path(archive), Path(output), Path(lock_path)
    if output.exists():
        raise ValueError(f"Output already exists; choose a new directory: {output}")
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    if lock.get("schema_version") != 1 or not lock.get("files"):
        raise ValueError("Unsupported or empty runtime lock")
    archive_bytes = archive.read_bytes()
    if digest(archive_bytes) != lock["archive_sha256"]:
        raise ValueError("Archive SHA-256 does not match the pinned upstream release")

    payloads = []
    names = set()
    with zipfile.ZipFile(archive) as source:
        for item in lock["files"]:
            name = item["runtime_path"]
            if (not name or name in (".", "..") or "/" in name or "\\" in name
                    or ":" in name or PurePosixPath(name).is_absolute() or name in names):
                raise ValueError(f"Unsafe or duplicate runtime filename: {name!r}")
            names.add(name)
            member = source.getinfo(item["archive_path"])
            if stat.S_ISLNK(member.external_attr >> 16) or member.is_dir():
                raise ValueError(f"Runtime member is not a regular file: {member.filename}")
            data = source.read(member)
            if len(data) != item["size"] or digest(data) != item["sha256"]:
                raise ValueError(f"Pinned file verification failed: {name}")
            payloads.append((name, data))

    # Validate every input before creating the destination. Write the manifest last;
    # a partial extraction is never a complete runtime and the host must reject it.
    output.parent.mkdir(parents=True, exist_ok=True)
    output.mkdir()
    for name, data in payloads:
        with (output / name).open("xb") as stream:
            stream.write(data)
    with (output / "runtime-manifest.json").open("x", encoding="utf-8") as stream:
        json.dump(lock, stream, indent=2)
        stream.write("\n")
    return {"prepared": True, "executed": False, "output": str(output.resolve()),
            "abi_profile": lock["abi_profile"], "files": len(payloads)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--lock", type=Path, default=DEFAULT_LOCK)
    args = parser.parse_args()
    try:
        result = prepare(args.archive, args.output, args.lock)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Runtime preparation failed: {error}\n")
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()

