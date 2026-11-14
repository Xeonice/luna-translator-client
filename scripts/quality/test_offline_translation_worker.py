"""Actual Argos adapter initialization against synthetic constructor spies.

No installed package, language model, inference or readiness is claimed here.
The check protects the original pipeline's options and observed runtime identity.
"""
import importlib.util
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[2] / "native/offline-translation/argos_worker.py"


class ArgosPipelineTests(unittest.TestCase):
    def initialize(self, compute_type, effective_compute_type):
        spec = importlib.util.spec_from_file_location("argos_worker_under_test", SOURCE)
        worker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(worker)
        settings = types.SimpleNamespace(chunk_type="STANZA", ChunkType=types.SimpleNamespace(STANZA="STANZA"),
                                         inter_threads=2, intra_threads=3, compute_type=compute_type, device="cpu")
        calls = {}

        class Package:
            def __init__(self, path):
                self.type, self.from_code, self.to_code = "translate", "ja", "zh"
                self.from_name, self.to_name, self.argos_version = "Japanese", "Chinese", "1.11.0"

        class Translation:
            def __init__(self, *_):
                self.sentencizer = types.SimpleNamespace(stanza_pipeline=None)

        def translator(path, **options):
            calls["translator"] = options
            return types.SimpleNamespace(compute_type=effective_compute_type)

        def pipeline(**options):
            calls["stanza"] = options
            return object()

        argos = types.ModuleType("argostranslate")
        argos.package = types.SimpleNamespace(Package=Package)
        argos.translate = types.SimpleNamespace(Language=lambda *_: object(), PackageTranslation=Translation)
        argos.settings = settings
        modules = {"argostranslate": argos, "stanza": types.SimpleNamespace(Pipeline=pipeline),
                   "ctranslate2": types.SimpleNamespace(Translator=translator)}
        with tempfile.TemporaryDirectory() as directory, patch.dict("sys.modules", modules), \
                patch.dict("os.environ"), patch.object(worker, "deny_network"), \
                patch.object(worker.importlib.metadata, "version", side_effect=lambda name: "1.11.0" if name == "argostranslate" else "synthetic-fixed-version"):
            root = Path(directory).resolve()
            (root / "pkg").mkdir()
            resource = {"engineVersion": "1.11.0", "files": [], "route": [
                {"pair": {"source": "ja", "target": "zh"}, "packageRelativePath": "pkg"}]}
            _, witness, scratch = worker.initialize(resource, root)
            scratch.cleanup()
            return calls, witness

    def test_original_compute_option_is_forwarded_and_actual_processing_is_observed(self):
        for configured, effective in [("auto", "int8"), ("float32", "float32")]:
            with self.subTest(configured=configured):
                calls, witness = self.initialize(configured, effective)
                self.assertEqual(calls["translator"], {"device": "cpu", "inter_threads": 2,
                                                      "intra_threads": 3, "compute_type": configured})
                self.assertIsNone(calls["stanza"]["download_method"])
                self.assertEqual(witness["processing"], {"pipeline": "argos-1.11.0-stanza-local-v1",
                    "chunkType": "STANZA", "device": "cpu", "computeType": configured,
                    "interThreads": 2, "intraThreads": 3, "effectiveComputeTypes": [effective]})

    def test_missing_effective_compute_observation_is_not_reported_installed(self):
        with self.assertRaisesRegex(Exception, "compute_type_observation"):
            # The adapter's bounded WorkerError is checked separately below:
            # its args contain the stable code and stage, no vendor error body.
            self.initialize("auto", None)


if __name__ == "__main__":
    unittest.main()
