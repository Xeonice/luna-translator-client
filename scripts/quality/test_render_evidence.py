"""Real offline browser guards. Success here never qualifies product UI."""
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import unittest
from unittest import mock
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent))
import render_evidence as render


def find_root():
    if os.environ.get("LUNA_RENDER_ROOT"):
        return Path(os.environ["LUNA_RENDER_ROOT"]).resolve()
    return next(p for p in Path(__file__).resolve().parents if (p / "scripts/quality/gate_core.py").is_file())


ROOT = find_root()
COLLECTOR = Path(os.environ.get("LUNA_RENDER_COLLECTOR", str(Path(__file__).resolve().parent.parent / "report/measure_render_evidence.mjs"))).resolve()
CSS = """@font-face{font-family:EvidenceMono;src:url('/fixture.ttf') format('truetype');font-weight:400}
*{box-sizing:border-box}html,body{margin:0;padding:0;font-family:EvidenceMono,monospace;font-size:16px;line-height:24px}
#app{width:1040px;padding:20px;background:#fff;color:#111}.layout{display:grid;grid-template-columns:1fr 1fr;gap:20px}
#source{font-size:23px;line-height:32px}#translation{font-size:18px;line-height:28px}
#source p{margin:0}rt{font-size:11px;line-height:14px}[data-pos]{color:rgb(20,80,140)}
@media(max-width:1100px){.layout{grid-template-columns:1fr}}
"""
RUBY = '<ruby><span data-token data-pos="noun">Tokyo</span><rt>toukyou</rt></ruby>'


class OutputPathSafetyTests(unittest.TestCase):
    """No browser/source freeze needed: output aliases must fail before any input read."""
    def setUp(self):
        self.run_dir = ROOT / ".build/acceptance-loop-20261105" / ("output-guards-" + uuid.uuid4().hex)
        self.real = self.run_dir / "owned-existing-parent"; self.real.mkdir(parents=True)
        for name in ("marker.txt", "measurement.json", "command-execution.json"):
            (self.real / name).write_text("retained original guard sentinel: " + name)
        self.originals = {p.name: p.read_bytes() for p in self.real.iterdir()}
        self.alias = self.run_dir / "alias-parent"; self.alias.symlink_to(self.real, target_is_directory=True)
        self.request = self.run_dir / "invalid-request.json"; self.request.write_text("{}\n")

    def assert_originals_unchanged(self):
        self.assertEqual({p.name: p.read_bytes() for p in self.real.iterdir()}, self.originals)

    def test_python_collect_rejects_alias_without_spawning(self):
        with mock.patch.object(render.subprocess, "run") as invoked:
            with self.assertRaisesRegex(ValueError, "symlink output ancestor"):
                render.collect(ROOT, self.request, self.alias / "collect-child", COLLECTOR)
            invoked.assert_not_called()
        self.assert_originals_unchanged()

    def test_python_recompute_rejects_alias_before_reading_original(self):
        with mock.patch.object(render, "inspect") as inspected, mock.patch.object(render, "collect") as invoked:
            with self.assertRaisesRegex(ValueError, "symlink output ancestor"):
                render.recompute(ROOT, self.real, self.alias / "recompute-child", COLLECTOR)
            inspected.assert_not_called(); invoked.assert_not_called()
        self.assert_originals_unchanged()

    def test_all_cli_modes_reject_alias_without_reading_or_writing_originals(self):
        for mode in ("measure", "recompute", "inspect"):
            command = [sys.executable, render.__file__, mode, "--root", str(ROOT), "--collector", str(COLLECTOR),
                       "--output", str(self.alias / (mode + "-child"))]
            command += ["--request", str(self.request)] if mode == "measure" else ["--evidence", str(self.real)]
            execution = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
            self.assertEqual(execution.returncode, 1, mode)
            self.assertNotIn("Traceback", execution.stderr)
            result = json.loads(execution.stdout)
            self.assertEqual(result["status"], "missing_render_evidence")
            self.assertIsNone(result["output"])
            self.assertTrue(any("symlink output ancestor" in e for e in result["errors"]))
            self.assert_originals_unchanged()

    def test_raw_parent_traversal_rejected_at_every_entry(self):
        output = self.run_dir / "unused" / ".." / "not-created"
        for call in (lambda: render.output_path(ROOT, output),
                     lambda: render.collect(ROOT, self.request, output, COLLECTOR),
                     lambda: render.recompute(ROOT, self.real, output, COLLECTOR)):
            with self.assertRaisesRegex(ValueError, "unsafe lexical output"):
                call()
        for mode in ("measure", "recompute", "inspect"):
            argv = [render.__file__, mode, "--root", str(ROOT), "--collector", str(COLLECTOR), "--output", str(output)]
            argv += ["--request", str(self.request)] if mode == "measure" else ["--evidence", str(self.real)]
            with mock.patch.object(sys, "argv", argv), mock.patch.object(render, "collect") as invoked, mock.patch.object(render, "inspect") as inspected:
                self.assertEqual(render.main(), 1)
                invoked.assert_not_called(); inspected.assert_not_called()
        command = ["node", str(COLLECTOR), "--root", str(ROOT), "--request", str(self.request), "--output", str(output)]
        execution = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(execution.returncode, 1)
        self.assertFalse(json.loads(execution.stdout)["output_created"])
        self.assertFalse((self.run_dir / "not-created").exists())
        self.assert_originals_unchanged()

    def test_containment_root_and_dangling_alias_are_rejected(self):
        dangling = self.run_dir / "dangling"; dangling.symlink_to(self.run_dir / "absent")
        for output in (ROOT / ".build", ROOT / ".build-other" / "not-created", dangling / "child"):
            with self.assertRaises(ValueError):
                render.output_path(ROOT, output)
        self.assert_originals_unchanged()

    def test_valid_fresh_lexical_output_keeps_actual_cli_ownership(self):
        output = self.run_dir / "fresh-owned-output"
        self.assertEqual(render.output_path(ROOT, output), output)
        execution = render.collect(ROOT, self.request, output, COLLECTOR)
        self.assertEqual(execution.returncode, 1)  # Deliberately invalid request, not product evidence.
        self.assertTrue(execution.output_created)
        self.assertEqual(render.load(output / "measurement.json")["status"], "failed")
        self.assertTrue((output / "command-execution.json").is_file())
        self.assert_originals_unchanged()


class OfflineRenderEvidenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.run_dir = ROOT / ".build/acceptance-loop-20261105" / ("guard-run-" + uuid.uuid4().hex)
        cls.inputs = cls.run_dir / "inputs"; cls.inputs.mkdir(parents=True)
        cls.font = cls.inputs / "fixture.ttf"
        # Retain actual local font bytes; no font or package download is performed.
        font = next((p for p in (Path("/System/Library/Fonts/Supplemental/Courier New.ttf"),
                                Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")) if p.is_file()), None)
        if font is None:
            raise RuntimeError("guard needs retained local TTF bytes; no font is downloaded")
        shutil.copyfile(font, cls.font)
        cls.epoch = render.source_identity(ROOT)

        def descriptor(file):
            return {"path": str(file.relative_to(ROOT)), "sha256": render.digest(file), "bytes": file.stat().st_size}

        def subject(name, *, source=RUBY, extra="", viewport=1040, scope="target", focus=False, form=False):
            css = cls.inputs / (name + ".css"); css.write_text(CSS + extra)
            html = cls.inputs / (name + ".html")
            html.write_text('<!doctype html><html><head><meta charset="utf-8"><link rel="stylesheet" href="/fixture.css"></head><body>'
                            '<main id="app"><div class="layout"><article id="source">' + source + '</article>'
                            '<article id="translation">Tokyo translation</article></div></main>'
                            '<script>document.querySelector("#source").textContent="SCRIPT RAN";fetch("https://example.invalid/")</script></body></html>')
            return {"id": name, "evidence_kind": "guard_fixture", "document": {"file": descriptor(html), "format": "html"},
                    "document_url": "https://retained.invalid/" + name, "viewport": {"width": viewport, "height": 760, "device_scale_factor": 1},
                    "assets": [{"url": "https://retained.invalid/fixture.css", "file": descriptor(css), "media_type": "text/css"},
                               {"url": "https://retained.invalid/fixture.ttf", "file": descriptor(cls.font), "media_type": "font/ttf"}],
                    "regions": {"window": "#app", "layout": ".layout", "source": "#source", "translation": "#translation"},
                    "token_selector": "[data-token]", "pos_selector": "[data-pos]", "media_queries": ["(max-width: 1100px)"],
                    "required_scope": scope, "required_properties": {"form_properties": form, "focus": focus},
                    "declared_state": {"showRuby": True, "showPos": True, "sourceFontPx": 23, "translationFontPx": 18}}

        subjects = [subject("reference"), subject("product"), subject("plain", source="<p>Tokyo</p>"),
                    subject("fonts", extra="#source{font-size:18px;line-height:26px}#translation{font-size:22px;line-height:34px}"),
                    subject("hidden", extra="rt{display:none}[data-pos]{color:inherit}"), subject("fake_client", viewport=1600),
                    subject("unlisted", extra="@import url('https://missing.invalid/a.css');"), subject("absent_asset"),
                    subject("substituted_css"), subject("whole_client", scope="whole_client"), subject("focus", focus=True),
                    subject("form", source=RUBY + '<input value="retained attribute">', form=True), subject("wrong_unit")]
        local = subject("local_font", extra="@font-face{font-family:LocalEvidence;src:local('Courier New')}#source{font-family:LocalEvidence,EvidenceMono}")
        local["evidence_kind"] = "immutable_snapshot"
        subjects.append(local)
        subjects[1]["regions"] = {"window": "main#app", "layout": "div.layout", "source": "article#source", "translation": "article#translation"}
        # @import must be first; otherwise browser ignores it before a resource request.
        unlisted = next(s for s in subjects if s["id"] == "unlisted")
        p = ROOT / unlisted["assets"][0]["file"]["path"]
        p.write_text("@import url('https://missing.invalid/a.css');\n" + CSS)
        unlisted["assets"][0]["file"] = descriptor(p)
        next(s for s in subjects if s["id"] == "absent_asset")["assets"][0]["file"]["path"] = str((cls.inputs / "absent.css").relative_to(ROOT))
        next(s for s in subjects if s["id"] == "substituted_css")["assets"][0]["file"]["sha256"] = "0" * 64
        next(s for s in subjects if s["id"] == "wrong_unit")["viewport"]["width"] = "1040px"
        cls.request = {"schema_version": 1, "kind": "offline-render-request/v1", "nonce": uuid.uuid4().hex,
                       "source_tree_sha256": cls.epoch["sha256"], "subjects": subjects,
                       "pairs": [["reference", s["id"]] for s in subjects if s["id"] != "reference"]}
        cls.request_file = cls.inputs / "request.json"; cls.request_file.write_text(json.dumps(cls.request, indent=2) + "\n")
        cls.evidence = cls.run_dir / "measurement"
        cls.execution = render.collect(ROOT, cls.request_file, cls.evidence, COLLECTOR)
        cls.report, cls.loaded_request, cls.inspect_errors = render.inspect(ROOT, cls.evidence, COLLECTOR)
        cls.result = render.compare(ROOT, cls.evidence, COLLECTOR)
        (cls.run_dir / "comparison.json").write_text(json.dumps(cls.result, indent=2) + "\n")
        cls.rows = {s["id"]: s for s in (cls.report or {}).get("subjects", [])}
        cls.pairs = {p["subjects"][1]: p for p in cls.result.get("pairs", [])}
        print("retained_guard_run=" + str(cls.run_dir), flush=True)

    def test_01_real_collection_and_independent_inspection(self):
        self.assertEqual(self.inspect_errors, [])
        self.assertEqual(self.execution.returncode, 1)  # Expected missing subjects keep batch non-green.
        self.assertEqual(self.report["status"], "missing_render_evidence")

    def test_02_self_contained_matching_target_is_comparable_only(self):
        self.assertEqual(self.pairs["product"]["status"], "comparable")
        self.assertFalse(self.pairs["product"]["whole_client_eligible"])
        self.assertTrue(all(self.result[k] is False for k in render.FLAGS))

    def test_03_page_script_and_external_fetch_never_run(self):
        self.assertEqual(self.rows["product"]["facts"]["regions"]["source"]["visible_text"], "Tokyo")
        self.assertEqual(self.rows["product"]["blocked_requests"], [])
        self.assertFalse(self.report["javascript_enabled"])

    def test_04_equal_raw_preferences_do_not_hide_p_vs_ruby(self):
        self.assertEqual(self.rows["plain"]["declared_state"], self.rows["reference"]["declared_state"])
        self.assertEqual(self.pairs["plain"]["status"], "incomparable")
        self.assertEqual(self.rows["plain"]["facts"]["regions"]["source"]["ruby_count"], 0)

    def test_05_effective_23_18_vs_18_22_rejected(self):
        self.assertEqual(self.pairs["fonts"]["status"], "incomparable")
        r = self.rows["fonts"]["facts"]["regions"]
        self.assertEqual([r[k]["style"]["font_size_px"] for k in ("source", "translation")], [18, 22])

    def test_06_hidden_rt_and_marker_without_visual_pos_rejected(self):
        source = self.rows["hidden"]["facts"]["regions"]["source"]
        self.assertTrue(source["ruby_count"] > 0 and len(source["rt"]) > 0 and len(source["pos"]) > 0)
        self.assertFalse(any(r["visible"] for r in source["rt"]))
        self.assertFalse(any(p["visual_effect"] for p in source["pos"]))
        self.assertEqual(self.pairs["hidden"]["status"], "incomparable")

    def test_07_fake_1040_window_at_1600_viewport_rejected(self):
        a, b = (self.rows[k]["facts"] for k in ("reference", "fake_client"))
        self.assertEqual(a["regions"]["window"]["rect"]["width"], b["regions"]["window"]["rect"]["width"])
        self.assertNotEqual(a["browser_viewport"], b["browser_viewport"])
        self.assertNotEqual(a["media"], b["media"])
        self.assertNotEqual(a["regions"]["layout"]["style"]["grid_template_columns"], b["regions"]["layout"]["style"]["grid_template_columns"])
        self.assertEqual(self.pairs["fake_client"]["status"], "incomparable")

    def test_08_unlisted_or_missing_asset_never_comparable(self):
        for name in ("unlisted", "absent_asset", "substituted_css"):
            self.assertEqual(self.pairs[name]["status"], "missing_render_evidence", name)
        self.assertTrue(self.rows["unlisted"]["blocked_requests"])

    def test_09_whole_client_forms_and_focus_unreconstructed(self):
        for name in ("whole_client", "form", "focus"):
            self.assertEqual(self.pairs[name]["status"], "missing_render_evidence", name)
            self.assertIsNone(self.rows[name]["granted_product_scope"])

    def test_10_wrong_units_not_silently_coerced(self):
        self.assertEqual(self.pairs["wrong_unit"]["status"], "missing_render_evidence")

    def test_11_nonfinite_json_rejected(self):
        bad = self.inputs / "nan-request.json"
        bad.write_text('{"viewport":NaN}')
        with self.assertRaisesRegex(ValueError, "nonfinite"):
            render.load(bad)
        output = self.run_dir / "nan-measurement"
        execution = render.collect(ROOT, bad, output, COLLECTOR)
        self.assertEqual(execution.returncode, 1)
        self.assertEqual(render.load(output / "measurement.json")["status"], "failed")

    def test_12_strict_false_not_boolean_lookalike(self):
        p = self.evidence / "measurement.json"; original = p.read_bytes()
        try:
            forged = json.loads(original); forged["completion_allowed"] = 0
            p.write_text(json.dumps(forged))
            self.assertTrue(render.inspect(ROOT, self.evidence, COLLECTOR)[2])
        finally:
            p.write_bytes(original)

    def test_13_forged_numeric_style_requires_actual_fresh_browser_replay(self):
        p = self.evidence / "measurement.json"; original = p.read_bytes()
        try:
            forged = json.loads(original)
            forged["subjects"][0]["facts"]["regions"]["source"]["style"]["font_size_px"] = 999
            p.write_text(json.dumps(forged))
            result = render.recompute(ROOT, self.evidence, self.run_dir / "fresh-replay", COLLECTOR)
            self.assertIn("retained facts differ from independent fresh browser replay", result["errors"])
            self.assertEqual(result["status"], "missing_render_evidence")
            (self.run_dir / "forged-style-recompute.json").write_text(json.dumps(result, indent=2) + "\n")
        finally:
            p.write_bytes(original)

    def test_14_wrong_unit_facts_rejected(self):
        p = self.evidence / "measurement.json"; original = p.read_bytes()
        try:
            forged = json.loads(original); forged["subjects"][0]["facts"]["regions"]["source"]["style"]["font_size_px"] = "23px"
            p.write_text(json.dumps(forged))
            self.assertTrue(render.inspect(ROOT, self.evidence, COLLECTOR)[2])
        finally:
            p.write_bytes(original)

    def test_15_missing_original_pack_can_only_report_missing(self):
        result = render.unavailable("legacy pack has no complete DOM/resource/font manifest")
        self.assertEqual(result["status"], "missing_render_evidence")
        self.assertTrue(all(result[k] is False for k in render.FLAGS))

    def test_16_clean_positive_cli_and_independent_replay(self):
        request = copy.deepcopy(self.request)
        request["subjects"] = request["subjects"][:2]
        request["pairs"] = [["reference", "product"]]
        request["nonce"] = uuid.uuid4().hex
        file = self.inputs / "positive-request.json"; file.write_text(json.dumps(request, indent=2) + "\n")
        evidence = self.run_dir / "positive-measurement"
        execution = render.collect(ROOT, file, evidence, COLLECTOR)
        self.assertEqual(execution.returncode, 0, execution.stderr)
        before = render.compare(ROOT, evidence, COLLECTOR)
        self.assertEqual(before["status"], "comparable")
        self.assertFalse(before["independently_recomputed"])
        result = render.recompute(ROOT, evidence, self.run_dir / "positive-replay", COLLECTOR)
        self.assertEqual(result["errors"], [])
        self.assertEqual(result["status"], "comparable")
        self.assertTrue(result["independently_recomputed"])
        self.assertTrue(all(result[k] is False for k in render.FLAGS))
        (self.run_dir / "positive-recomputed.json").write_text(json.dumps(result, indent=2) + "\n")

    def test_17_retained_css_change_after_collection_rejected(self):
        css = ROOT / self.request["subjects"][1]["assets"][0]["file"]["path"]
        original = css.read_bytes()
        try:
            css.write_bytes(original + b"\n/* changed retained bytes */\n")
            self.assertTrue(render.inspect(ROOT, self.evidence, COLLECTOR)[2])
        finally:
            css.write_bytes(original)

    def test_18_false_source_epoch_rejected(self):
        p = self.evidence / "measurement.json"; original = p.read_bytes()
        try:
            forged = json.loads(original)
            forged["source_before"]["sha256"] = "0" * 64
            forged["source_after"]["sha256"] = "0" * 64
            p.write_text(json.dumps(forged))
            self.assertIn("managed source drift", render.inspect(ROOT, self.evidence, COLLECTOR)[2])
        finally:
            p.write_bytes(original)

    def test_19_existing_output_is_rejected_without_overwriting_originals(self):
        files = {name: (self.evidence / name).read_bytes() for name in ("measurement.json", "command-execution.json")}
        with self.assertRaisesRegex(ValueError, "fresh ignored output"):
            render.collect(ROOT, self.request_file, self.evidence, COLLECTOR)
        command = ["node", str(COLLECTOR), "--root", str(ROOT), "--request", str(self.request_file), "--output", str(self.evidence)]
        execution = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(execution.returncode, 1)
        for name, original in files.items():
            self.assertEqual((self.evidence / name).read_bytes(), original, name)

    def test_20_invalid_original_cli_returns_missing_without_crash(self):
        output = self.run_dir / "invalid-original-recompute"
        command = [sys.executable, str(Path(render.__file__)), "recompute", "--root", str(ROOT), "--collector", str(COLLECTOR),
                   "--evidence", str(self.run_dir / "absent-original"), "--output", str(output)]
        execution = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(execution.returncode, 1)
        self.assertNotIn("Traceback", execution.stderr)
        result = render.load(output / "comparison.json")
        self.assertEqual(result["status"], "missing_render_evidence")
        self.assertFalse(result["independently_recomputed"])

    def test_21_replaced_png_with_updated_hash_rejected_by_fresh_replay(self):
        evidence = self.run_dir / "positive-measurement"
        report_path = evidence / "measurement.json"; original_report = report_path.read_bytes()
        report = json.loads(original_report)
        screenshot = evidence / report["subjects"][0]["screenshot"]["path"]
        original_png = screenshot.read_bytes()
        try:
            alternate = self.evidence / self.rows["hidden"]["screenshot"]["path"]
            screenshot.write_bytes(alternate.read_bytes())
            report["subjects"][0]["screenshot"].update({"sha256": render.digest(screenshot), "bytes": screenshot.stat().st_size})
            report_path.write_text(json.dumps(report))
            result = render.recompute(ROOT, evidence, self.run_dir / "replaced-png-replay", COLLECTOR)
            self.assertEqual(result["status"], "missing_render_evidence")
            self.assertIn("retained facts differ from independent fresh browser replay", result["errors"])
            (self.run_dir / "replaced-png-recompute.json").write_text(json.dumps(result, indent=2) + "\n")
        finally:
            screenshot.write_bytes(original_png); report_path.write_bytes(original_report)

    def test_22_local_font_resolution_cannot_claim_fixed_original_bytes(self):
        self.assertEqual(self.pairs["local_font"]["status"], "missing_render_evidence")
        self.assertIn("local_font_resolution_not_attested", self.rows["local_font"]["gaps"])

    def test_23_cli_does_not_read_or_write_racing_collectors_directory(self):
        output = self.run_dir / "raced-main"
        def lost_ownership(*args):
            output.mkdir(); (output / "foreign.txt").write_text("another collector")
            execution = subprocess.CompletedProcess([], 1, stdout="", stderr="")
            execution.output_created = False
            return execution
        argv = [render.__file__, "measure", "--root", str(ROOT), "--collector", str(COLLECTOR),
                "--request", str(self.request_file), "--output", str(output)]
        with mock.patch.object(sys, "argv", argv), mock.patch.object(render, "collect", side_effect=lost_ownership), mock.patch.object(render, "compare") as compare:
            self.assertEqual(render.main(), 1)
            compare.assert_not_called()
        self.assertEqual([p.name for p in output.iterdir()], ["foreign.txt"])

    def test_24_recompute_does_not_read_racing_collectors_receipt(self):
        output = self.run_dir / "raced-recompute"
        def lost_ownership(*args):
            output.mkdir(); (output / "foreign.txt").write_text("another collector")
            execution = subprocess.CompletedProcess([], 1, stdout="", stderr="")
            execution.output_created = False
            return execution
        with mock.patch.object(render, "inspect", return_value=(self.report, self.request, [])), mock.patch.object(render, "collect", side_effect=lost_ownership), mock.patch.object(render, "compare") as compare:
            result = render.recompute(ROOT, self.evidence, output, COLLECTOR)
            self.assertEqual(result["status"], "missing_render_evidence")
            self.assertFalse(result["collection"]["output_owned"])
            compare.assert_not_called()
        self.assertEqual([p.name for p in output.iterdir()], ["foreign.txt"])

    def test_25_direct_collector_cannot_follow_symlink_output_ancestor(self):
        real = self.run_dir / "real-output-parent"; real.mkdir()
        alias = self.run_dir / "alias-output-parent"; alias.symlink_to(real, target_is_directory=True)
        command = ["node", str(COLLECTOR), "--root", str(ROOT), "--request", str(self.request_file), "--output", str(alias / "child")]
        execution = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(execution.returncode, 1)
        self.assertFalse(json.loads(execution.stdout)["output_created"])
        self.assertEqual(list(real.iterdir()), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
