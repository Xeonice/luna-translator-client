#!/usr/bin/env python3
"""translation-worker-v1, external Argos 1.11.0 / package-local Stanza pipeline.

No interpreter, engine or language package is bundled or installed by this file.
No automatic language discovery, pivot, package index, download or cloud route.
Protocol output is isolated from dependency stdout. Version-coupled adapter code
is original GPL-3.0-or-later; upstream Argos is MIT (see README).
"""
import contextlib
import hashlib
import importlib.metadata
import io
import json
import os
from pathlib import Path
import platform
import socket
import sys
import tempfile

PROTOCOL = "translation-worker-v1"
MAX_FRAME = 1048576
MAX_TEXT = 65536
MAX_OUTPUT = 262144
WIRE_OUT = sys.stdout.buffer


class WorkerError(Exception):
    def __init__(self, code, stage):
        self.code, self.stage = code, stage


def read_frame():
    data = sys.stdin.buffer.readline(MAX_FRAME + 1)
    if not data or len(data) > MAX_FRAME or not data.endswith(b"\n"):
        raise WorkerError("protocol_violation", "frame_budget")
    def unique(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise WorkerError("protocol_violation", "duplicate_field")
            value[key] = item
        return value
    try:
        value = json.loads(data, object_pairs_hook=unique)
    except (ValueError, UnicodeError):
        raise WorkerError("protocol_violation", "decode")
    if not isinstance(value, dict) or value.get("protocol") != PROTOCOL:
        raise WorkerError("protocol_violation", "protocol")
    return value


def emit(value):
    data = json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
    if len(data) > MAX_FRAME:
        raise WorkerError("output_too_large", "frame_budget")
    WIRE_OUT.write(data)
    WIRE_OUT.flush()


def checked_path(root, relative):
    p = Path(relative)
    if not relative or "\\" in relative or "\0" in relative or p.is_absolute() or any(i in (".", "..") for i in relative.split("/")):
        raise WorkerError("invalid_resource", "resource_path")
    candidate = root
    for part in p.parts:
        candidate = candidate / part
        if candidate.is_symlink():
            raise WorkerError("invalid_resource", "resource_symlink")
    if not candidate.exists() or not candidate.resolve().is_relative_to(root):
        raise WorkerError("missing_resource", "resource_path")
    return candidate


def verify(resource):
    root = Path(resource["root"])
    if not root.is_absolute() or root.resolve() != root or not root.is_dir():
        raise WorkerError("invalid_resource", "resource_root")
    files = resource["files"]
    if not files or len(files) > 16384:
        raise WorkerError("invalid_resource", "resource_budget")
    seen, total = set(), 0
    for spec in files:
        relative = spec["relativePath"]
        if relative in seen:
            raise WorkerError("invalid_resource", "duplicate_resource")
        seen.add(relative)
        path = checked_path(root, relative)
        length = path.stat().st_size
        total += length
        if total > 34359738368 or length != spec["length"] or not path.is_file():
            raise WorkerError("resource_changed", "resource_length")
        digest = hashlib.sha256()
        with path.open("rb") as handle:
            while chunk := handle.read(65536):
                digest.update(chunk)
        if digest.hexdigest() != spec["sha256"]:
            raise WorkerError("resource_changed", "resource_digest")
    digest = hashlib.sha256(json.dumps([files, resource["route"], resource["engineVersion"], resource["identity"]], ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    if digest != resource["digest"]:
        raise WorkerError("resource_changed", "manifest_digest")
    return root


def deny_network():
    # Stanza is also explicitly configured not to download. Block accidental
    # Python-level dependency connections rather than merely setting proxy env.
    class OfflineSocket(socket.socket):
        def connect(self, address):
            raise WorkerError("permission_denied", "network_forbidden")
        def connect_ex(self, address):
            raise WorkerError("permission_denied", "network_forbidden")
    socket.socket = OfflineSocket
    def blocked(*args, **kwargs):
        raise WorkerError("permission_denied", "network_forbidden")
    socket.create_connection = blocked


def initialize(resource, root):
    if resource["engineVersion"] != "1.11.0" or importlib.metadata.version("argostranslate") != "1.11.0":
        raise WorkerError("runtime_incompatible", "argos_version")
    versions = {name: importlib.metadata.version(name) for name in ["argostranslate", "ctranslate2", "sentencepiece", "stanza", "minisbd", "packaging"]}
    versions["python"] = platform.python_version()
    # No user settings, implicit model fetch, GPU selection or external package
    # search. Own temporary cache is cleaned at process exit, not the install.
    scratch = tempfile.TemporaryDirectory(prefix="luna-argos-")
    try:
        return initialize_with_scratch(resource, root, versions, scratch)
    except BaseException:
        scratch.cleanup()
        raise


def initialize_with_scratch(resource, root, versions, scratch):
    for name in ["XDG_DATA_HOME", "XDG_CACHE_HOME", "XDG_CONFIG_HOME"]:
        os.environ[name] = scratch.name
    os.environ["ARGOS_CHUNK_TYPE"] = "STANZA"
    os.environ["ARGOS_DEVICE_TYPE"] = "cpu"
    os.environ["ARGOS_MODEL_PROVIDER"] = "OPENNMT"
    os.environ["ARGOS_DEBUG"] = "0"
    os.environ["ARGOS_PACKAGE_DIR"] = str(root)
    os.environ["HF_HUB_OFFLINE"] = "1"
    deny_network()
    import stanza
    from argostranslate import package, translate, settings
    if settings.chunk_type != settings.ChunkType.STANZA:
        raise WorkerError("runtime_incompatible", "argos_pipeline")
    versions["processing"] = {
        "pipeline": "argos-1.11.0-stanza-local-v1", "chunkType": "STANZA",
        "device": settings.device, "computeType": settings.compute_type,
        "interThreads": settings.inter_threads, "intraThreads": settings.intra_threads,
        "effectiveComputeTypes": [],
    }
    if settings.device != "cpu":
        raise WorkerError("runtime_incompatible", "argos_device")
    engines = []
    previous = None
    files = {s["relativePath"] for s in resource["files"]}
    for hop in resource["route"]:
        pair = hop["pair"]
        relative = hop["packageRelativePath"]
        package_path = checked_path(root, relative)
        if previous is not None and pair["source"] != previous:
            raise WorkerError("invalid_resource", "route")
        previous = pair["target"]
        pkg = package.Package(package_path)
        if pkg.type != "translate" or pkg.from_code != pair["source"] or pkg.to_code != pair["target"]:
            raise WorkerError("unsupported_pair", "package_pair")
        if pkg.argos_version and not pkg.argos_version.startswith(("1.",)):
            raise WorkerError("runtime_incompatible", "package_version")
        # Verify all package files are in the frozen snapshot. A directory-only
        # witness or a newly inserted tokenizer/model cannot be silently used.
        for actual in package_path.rglob("*"):
            if actual.is_symlink() or (actual.is_file() and actual.relative_to(root).as_posix() not in files):
                raise WorkerError("resource_changed", "unsealed_package_file")
        from_lang = translate.Language(pkg.from_code, pkg.from_name)
        to_lang = translate.Language(pkg.to_code, pkg.to_name)
        engine = translate.PackageTranslation(from_lang, to_lang, pkg)
        # Argos 1.11.0 StanzaSentencizer.lazy_pipeline otherwise uses Stanza's
        # default download mode. Keep the original packaged sentence/tokenize/
        # CTranslate2/paragraph pipeline and pre-create only its local tokenizer.
        language = {"zt": "zh-hant", "pb": "pt"}.get(pkg.from_code, pkg.from_code)
        engine.sentencizer.stanza_pipeline = stanza.Pipeline(
            lang=language, dir=str(package_path / "stanza"), processors="tokenize",
            use_gpu=False, logging_level="ERROR", download_method=None,
        )
        import ctranslate2
        engine.translator = ctranslate2.Translator(
            str(package_path / "model"), device=settings.device,
            inter_threads=settings.inter_threads, intra_threads=settings.intra_threads,
            compute_type=settings.compute_type,
        )
        effective_compute = getattr(engine.translator, "compute_type", None)
        if not isinstance(effective_compute, str) or not effective_compute:
            raise WorkerError("runtime_incompatible", "compute_type_observation")
        versions["processing"]["effectiveComputeTypes"].append(effective_compute)
        engines.append(engine)
    if not 1 <= len(engines) <= 4:
        raise WorkerError("invalid_resource", "route")
    return engines, versions, scratch


def main():
    hello = read_frame()
    instance, request_id = hello.get("workerInstanceId", ""), hello.get("requestId", "")
    try:
        if hello.get("kind") != "hello" or hello.get("engineId") != "argos_translate" or hello.get("contractRevision") != 1 or not instance or not request_id:
            raise WorkerError("protocol_violation", "hello")
        resource = hello["resource"]
        root = verify(resource)
        with contextlib.redirect_stdout(sys.stderr):
            engines, versions, scratch = initialize(resource, root)
        observation = {"engineId": "argos_translate", "engineContractRevision": 1, "runtimeVersion": json.dumps(versions, sort_keys=True, separators=(",", ":")), "platform": sys.platform, "architecture": platform.machine(), "availability": "installed", "physicalCancelSupported": False, "resourceVersion": "1.11.0", "reason": None}
        emit({"kind": "ready", "protocol": PROTOCOL, "workerInstanceId": instance, "requestId": request_id, "resourceDigest": resource["digest"], "observation": observation})
        if hello.get("operation") == "observe":
            return
        if hello.get("operation") != "translate":
            raise WorkerError("requires_interactive_system_host", "prepare_not_implicit")
        execute = read_frame()
        if execute.get("kind") == "cancel":
            raise WorkerError("cancelled", "before_execute")
        identity = hello.get("identity")
        if execute.get("kind") != "execute" or execute.get("workerInstanceId") != instance or execute.get("identity") != identity or not identity:
            raise WorkerError("protocol_violation", "execute_identity")
        text = execute["sourceText"]
        if not isinstance(text, str) or not text or "\0" in text or len(text.encode()) > MAX_TEXT or hashlib.sha256(text.encode()).hexdigest() != identity["sourceHash"]:
            raise WorkerError("invalid_request", "input")
        with contextlib.redirect_stdout(sys.stderr):
            for engine in engines:
                text = engine.translate(text)
                if not text or "\0" in text or len(text.encode()) > MAX_OUTPUT:
                    raise WorkerError("output_too_large", "output")
        verify(resource)
        emit({"kind": "final", "protocol": PROTOCOL, "workerInstanceId": instance, "identity": identity, "resourceIdentity": resource["identity"], "resourceDigest": resource["digest"], "route": resource["route"], "text": text})
        scratch.cleanup()
    except WorkerError as error:
        emit({"kind": "error", "protocol": PROTOCOL, "workerInstanceId": instance, "requestId": request_id, "error": {"code": error.code, "stage": error.stage}})
    except importlib.metadata.PackageNotFoundError:
        emit({"kind": "error", "protocol": PROTOCOL, "workerInstanceId": instance, "requestId": request_id, "error": {"code": "not_installed", "stage": "python_dependency"}})
    except Exception:
        # Dependency exceptions can include source text, paths or vendor data.
        emit({"kind": "error", "protocol": PROTOCOL, "workerInstanceId": instance, "requestId": request_id, "error": {"code": "engine_failed", "stage": "argos_pipeline"}})


if __name__ == "__main__":
    try:
        main()
    except Exception:
        sys.exit(2)
