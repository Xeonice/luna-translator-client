"""Positive and mutation tests for offline specification/evidence contracts."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

_SOURCE = Path(__file__).with_name("spec_contract.py")
_SPEC = importlib.util.spec_from_file_location("quality_spec_contract", _SOURCE)
assert _SPEC and _SPEC.loader
contract = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(contract)


class ContractTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.change_id = "enforce-engineering-contracts"
        self.prefix = f"openspec/changes/{self.change_id}"
        self.write(f"{self.prefix}/proposal.md", "# Proposal\nTooling only.\n")
        self.write(f"{self.prefix}/design.md", "# Design\nNo product conformance claim.\n")
        self.write(f"{self.prefix}/tasks.md", "- [ ] Implement checks\n")
        self.write(f"{self.prefix}/source-snapshot.json", "{}\n")
        self.write(f"{self.prefix}/specs/engineering-enforcement/spec.md", """## ADDED Requirements
### Requirement: ENG-001 Require traceable checks
The gate SHALL require mappings.
#### Scenario: A declared check is mapped
- **WHEN** a check is declared
- **THEN** its scenario is mapped
""")
        self.document = {
            "schema_version": 1,
            "id": self.change_id,
            "kind": "tooling",
            "allowed_paths": [f"{self.prefix}/**", "openspec/specs/**", "openspec/policy.json", "scripts/quality/**"],
            "source_snapshot": f"{self.prefix}/source-snapshot.json",
            "requirements": [{"id": "ENG-001", "spec": "specs/engineering-enforcement/spec.md", "scenario": "A declared check is mapped", "checks": ["guard-unit-tests"]}],
        }
        self.save()

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def save(self):
        self.write(f"{self.prefix}/verification.json", json.dumps(self.document))

    def approved_design(self):
        from design_authority import (ENTRY, RECORD, CURRENT_RECORD,
                                      current_source_approval)
        source_root = _SOURCE.parents[2]
        approval = current_source_approval(source_root)
        self.assertEqual('approved_by_user', approval['status'])
        paths = {RECORD, CURRENT_RECORD}
        for key in ['design_sources', 'runtime_dependencies', 'authority_dependencies']:
            paths.update(approval[key])
        for relative in paths:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_root / relative, target)
        return ENTRY

    def errors(self):
        self.save()
        return contract.validate_change(self.root, self.change_id)

    def product(self):
        self.document["kind"] = "product"
        self.write("docs/architecture/engineering-modules.csv", "module_id,name\nM01,library\n")
        self.write("docs/architecture/engineering-components.csv", "component_id,name\nC01,MaterialSurface\n")
        self.write("docs/product/first-product-scope.md", "| FP-03 | Target selection |\n")
        self.write("docs/product/model-configuration-prd.md", "### AI-CFG-01 Service connection\n")
        self.write("docs/product/ai-reading-prd.md", "| AI-SC-01 Translation |\n")
        self.write("docs/product/model-configuration-acceptance.csv", "acceptance_id\nMC-01\n")
        self.write("docs/product/game-library-window-acceptance.csv", "case_id\nGL-01\n")
        self.write("docs/product/reading-window-session-acceptance.csv", "case_id\nRW-01\n")
        self.write("docs/product/reading-window-session-prd.md", "# Reading window product requirements\n")
        self.write("docs/development/requirements.csv", "requirement_id\nREQ-001\n")
        self.write("docs/development/client-learning-extensions.csv", "extension_id\nEXT-JA-001\n")
        entry = self.approved_design()
        delta = self.root / self.prefix / "specs/engineering-enforcement/spec.md"
        delta.write_text(delta.read_text().replace("ENG-001", "FP-03"))
        self.document["requirements"][0].update({"id": "FP-03", "source_refs": ["docs/product/first-product-scope.md"], "design_refs": [entry + "#main-content"], "module_ids": ["M01"], "component_ids": ["C01"]})

    def test_tooling_mapping_passes_without_claiming_tasks_complete(self):
        self.assertEqual([], self.errors())

    def test_actual_current_composition_entries_and_literal_anchors_are_supported(self):
        from design_authority import ENTRY, HISTORY_ROOT
        errors = []
        root = _SOURCE.parents[2]
        contract._design_reference(root, ENTRY + '#main-content', 'guided', errors)
        contract._design_reference(root, HISTORY_ROOT + '/index.html#settings-window', 'history', errors)
        self.assertEqual([], errors)

    def test_arbitrary_candidate_cannot_become_authority_by_matching_an_anchor(self):
        path = 'docs/design/first-product-v1/candidates/unreviewed/index.html'
        self.write(path, '<main id="main-content"></main>')
        errors = []
        contract._design_reference(self.root, path + '#main-content', 'candidate', errors)
        self.assertTrue(errors)
        self.assertIn('current code-owned design entry', errors[0])

    def test_root_html_cannot_bypass_source_approval_for_a_new_product_change(self):
        self.product()
        for name in ['new-unreviewed.html', 'index.html', 'interaction-review.html']:
            relative = 'docs/design/first-product-v1/' + name
            self.write(relative, '<main id="main-content"></main>')
            self.document['requirements'][0]['design_refs'] = [relative + '#main-content']
            self.assertTrue(any('current code-owned design entry' in error for error in self.errors()))

    def test_historical_compatibility_requires_exact_metadata_and_source_bytes(self):
        source_root = _SOURCE.parents[2]
        registry = json.loads(_SOURCE.with_name('legacy_product_references.json').read_text())
        change_id, frozen = next(iter(registry['changes'].items()))
        paths = [f'openspec/changes/{change_id}/verification.json', *frozen['design_sources'], *frozen['change_sources']]
        for relative in paths:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source_root / relative, target)
        refs = contract._historical_design_references(self.root.resolve(), change_id)
        self.assertEqual(frozenset(frozen['references']), refs)
        for ref in refs:
            errors = []
            contract._design_reference(self.root, ref, 'historical', errors, historical=refs)
            self.assertEqual([], errors)
        self.assertEqual(frozenset(), contract._historical_design_references(self.root, 'forged-new-change'))
        path = self.root / paths[0]
        original = path.read_bytes()
        path.write_bytes(original + b' ')
        self.assertEqual(frozenset(), contract._historical_design_references(self.root, change_id))
        path.write_bytes(original)
        proposal = self.root / f'openspec/changes/{change_id}/proposal.md'
        original_proposal = proposal.read_bytes()
        proposal.write_bytes(original_proposal + b'\nA different new product.\n')
        self.assertEqual(frozenset(), contract._historical_design_references(self.root, change_id))
        proposal.write_bytes(original_proposal)
        added = proposal.with_name('new-unreviewed-spec.md')
        added.write_text('New scope')
        self.assertEqual(frozenset(), contract._historical_design_references(self.root, change_id))
        added.unlink()
        source = self.root / next(iter(frozen['design_sources']))
        source.write_bytes(source.read_bytes() + b'<!-- drift -->')
        self.assertEqual(frozenset(), contract._historical_design_references(self.root, change_id))

    def test_historical_registry_is_code_pinned(self):
        with patch.object(contract, '_LEGACY_REFERENCE_SHA256', '0' * 64):
            self.assertEqual(frozenset(), contract._historical_design_references(_SOURCE.parents[2], 'implement-first-product'))

    def test_current_entry_requires_full_approved_source_closure_not_only_valid_anchor(self):
        ENTRY = self.approved_design()
        valid = []
        contract._design_reference(self.root, ENTRY + '#main-content', 'current', valid)
        self.assertEqual([], valid)
        target = self.root / ENTRY
        target.write_text(target.read_text() + '<!-- unapproved drift -->')
        errors = []
        contract._design_reference(self.root, ENTRY + '#main-content', 'current', errors)
        self.assertTrue(errors)
        self.assertIn('source approval', errors[0])

    def test_product_requires_real_references_and_known_matrix_ids(self):
        self.product()
        self.assertEqual([], self.errors())
        self.document["requirements"][0]["module_ids"] = ["M99"]
        self.assertTrue(any("unknown ID (M99)" in error for error in self.errors()))

    def test_product_missing_component_and_design_mappings_fail(self):
        self.product()
        del self.document["requirements"][0]["component_ids"]
        del self.document["requirements"][0]["design_refs"]
        errors = self.errors()
        self.assertTrue(any("component_ids" in error for error in errors))
        self.assertTrue(any("design_refs" in error for error in errors))

    def test_product_design_requires_a_real_literal_anchor(self):
        from design_authority import ENTRY
        self.product()
        self.assertEqual([], self.errors())
        for reference in (
            ENTRY + "#missing",
            ENTRY + "#main-content .child",
            ENTRY + "#",
        ):
            with self.subTest(reference=reference):
                self.document["requirements"][0]["design_refs"] = [reference]
                self.assertTrue(any("design_refs" in error for error in self.errors()))

    def test_product_design_rejects_old_topics_archives_and_reports(self):
        self.product()
        for path in (
            "docs/design/archive/old-topic/index.html",
            "docs/design/old-topic/index.html",
            "reports/old-design.html",
        ):
            with self.subTest(path=path):
                self.write(path, '<main id="library"></main>')
                self.document["requirements"][0]["design_refs"] = [path + "#library"]
                self.assertTrue(any("must reference current" in error for error in self.errors()))

    def test_product_design_dynamic_placeholder_is_not_a_literal_anchor(self):
        path = self.write("fixture.html", '<script>const markup = \'<main id="library"></main>\';</script>')
        errors = []
        contract._literal_design_anchor(path, 'library', 'fixture', errors)
        self.assertTrue(any("missing literal HTML id/name" in error for error in errors))

    def test_product_design_accepts_literal_name_anchor(self):
        path = self.write("fixture.html", '<a name="library"></a>')
        errors = []
        contract._literal_design_anchor(path, 'library', 'fixture', errors)
        self.assertEqual([], errors)

    def test_inert_markup_and_input_names_are_not_design_fragment_targets(self):
        for html in ['<template><main id="library"></main></template>', '<noscript><main id="library"></main></noscript>', '<input name="library">']:
            with self.subTest(html=html):
                path = self.write('fixture.html', html)
                errors = []
                contract._literal_design_anchor(path, 'library', 'fixture', errors)
                self.assertTrue(any('missing literal HTML' in error for error in errors))

    def test_unplanned_product_requirement_fails_with_other_references_present(self):
        self.product()
        delta = self.root / self.prefix / "specs/engineering-enforcement/spec.md"
        for identifier in ("FP-99", "FP-99-cloud-sync"):
            with self.subTest(identifier=identifier):
                delta.write_text(delta.read_text().replace(self.document["requirements"][0]["id"], identifier))
                self.document["requirements"][0]["id"] = identifier
                self.assertTrue(any("unknown product requirement" in error for error in self.errors()))

    def test_explicit_source_ids_must_be_known_and_readme_is_not_authority(self):
        self.product()
        self.document["requirements"][0]["source_requirement_ids"] = ["FP-99"]
        self.assertTrue(any("unknown authoritative" in error for error in self.errors()))
        self.document["requirements"][0]["source_requirement_ids"] = ["FP-03"]
        self.write("README.md", "# Product\n")
        self.document["requirements"][0]["source_refs"] = ["README.md"]
        self.assertTrue(any("README alone" in error for error in self.errors()))

    def test_missing_source_registry_fails_closed(self):
        self.product()
        (self.root / "docs/development/requirements.csv").unlink()
        self.assertTrue(any("product source catalog" in error for error in self.errors()))

    def test_reading_window_id_and_prd_are_authoritative(self):
        self.product()
        self.document["requirements"][0]["source_requirement_ids"] = ["RW-01"]
        self.document["requirements"][0]["source_refs"] = ["docs/product/reading-window-session-prd.md"]
        self.assertEqual([], self.errors())
        self.document["requirements"][0]["source_requirement_ids"] = ["RW-99"]
        self.assertTrue(any("unknown authoritative" in error for error in self.errors()))

    def test_missing_or_duplicate_reading_window_catalog_fails_closed(self):
        self.product()
        path = self.root / "docs/product/reading-window-session-acceptance.csv"
        path.write_text("case_id\nRW-01\nRW-01\n")
        self.assertTrue(any("duplicate authoritative" in error for error in self.errors()))
        path.unlink()
        self.assertTrue(any("product source catalog" in error for error in self.errors()))

    def test_missing_requirement_mapping_fails_reverse_coverage(self):
        self.document["requirements"] = []
        self.assertTrue(any("unmapped delta Scenario" in error for error in self.errors()))

    def test_undeclared_extra_scenario_or_spec_fails(self):
        self.write(f"{self.prefix}/specs/engineering-enforcement/spec.md", (self.root / self.prefix / "specs/engineering-enforcement/spec.md").read_text() + "\n#### Scenario: An omitted negative case\n- **WHEN** mapping is absent\n- **THEN** reject\n")
        self.assertTrue(any("An omitted negative case" in error for error in self.errors()))
        self.write(f"{self.prefix}/specs/another/spec.md", "### Requirement: ENG-002 Another rule\n#### Scenario: Not mapped\n")
        self.assertTrue(any("ENG-002" in error for error in self.errors()))

    def test_markdown_fence_length_does_not_hide_unmapped_requirements(self):
        delta = self.root / self.prefix / "specs/engineering-enforcement/spec.md"
        delta.write_text(delta.read_text() + "\n````text\n```\n````\n### Requirement: ENG-999 Omitted\n#### Scenario: Not inside the fence\n")
        self.assertTrue(any("ENG-999" in error for error in self.errors()))

    def test_fenced_example_headings_are_not_actual_delta_requirements(self):
        delta = self.root / self.prefix / "specs/engineering-enforcement/spec.md"
        delta.write_text(delta.read_text() + "\n~~~markdown\n### Requirement: EXAMPLE-001 Example\n#### Scenario: Only an example\n~~~\n")
        self.assertEqual([], self.errors())

    def test_incorrect_id_heading_and_scenario_fail(self):
        self.document["requirements"][0]["id"] = "ENG-002"
        self.assertTrue(any("match exactly one" in error for error in self.errors()))
        self.document["requirements"][0]["scenario"] = "Wrong scenario"
        self.assertTrue(any("match exactly one" in error for error in self.errors()))

    def test_duplicate_mapping_and_empty_check_fail(self):
        self.document["requirements"].append(dict(self.document["requirements"][0]))
        self.document["requirements"][0]["checks"] = []
        errors = self.errors()
        self.assertTrue(any("duplicate spec" in error for error in errors))
        self.assertTrue(any("checks: expected a nonempty list" in error for error in errors))

    def test_snapshot_escape_and_other_change_fail(self):
        self.document["source_snapshot"] = "../outside.json"
        self.document["allowed_paths"].append("openspec/changes/other-change/**")
        errors = self.errors()
        self.assertTrue(any("source_snapshot" in error for error in errors))
        self.assertTrue(any("another change" in error for error in errors))

    def test_broad_and_unsupported_globs_fail(self):
        for pattern in ["**", "*", "apps/**", "crates/**", "docs/**", "scripts/**", "openspec/**", "scripts/quality/*.py", "../outside/**"]:
            with self.subTest(pattern=pattern):
                self.document["allowed_paths"] = [pattern]
                self.assertTrue(self.errors())

    def test_tooling_may_not_grant_product_files(self):
        self.document["allowed_paths"].append("apps/desktop/src/App.tsx")
        self.assertTrue(any("tooling changes" in error for error in self.errors()))

    def test_symlinked_source_or_delta_is_rejected(self):
        self.product()
        original = self.root / "docs/product/first-product-scope.md"
        original.unlink()
        original.symlink_to(self.write("real-scope.md", "# Scope\n"))
        self.assertTrue(any("symbolic links" in error for error in self.errors()))

    def test_change_id_traversal_rejected(self):
        self.assertTrue(contract.validate_change(self.root, "../outside"))

    def test_unhashable_kind_is_a_validation_error_not_a_crash(self):
        for value in ([], {}):
            with self.subTest(value=value):
                self.document["kind"] = value
                self.assertTrue(any("kind:" in error for error in self.errors()))

    def test_symlinked_hidden_delta_directory_is_rejected(self):
        hidden = self.root / "hidden-deltas"
        hidden.mkdir()
        (hidden / "spec.md").write_text("### Requirement: ENG-999 Hidden\n#### Scenario: Hidden scenario\n")
        (self.root / self.prefix / "specs/hidden").symlink_to(hidden, target_is_directory=True)
        self.assertTrue(any("symbolic links" in error for error in self.errors()))

    def test_invalid_json_constants_and_duplicate_keys_rejected(self):
        path = self.root / self.prefix / "verification.json"
        for invalid in ('{"schema_version": NaN}', '{"schema_version": 1, "schema_version": 1}'):
            path.write_text(invalid)
            self.assertTrue(any("invalid JSON" in error for error in contract.validate_change(self.root, self.change_id)))

    def test_false_verified_claim_with_all_unchecked_tasks_rejected(self):
        self.document["status"] = "verified"
        self.assertTrue(any("all unchecked" in error for error in self.errors()))

    def test_cli_is_nonzero_on_failure_and_json_is_machine_readable(self):
        self.document["requirements"] = []
        self.save()
        result = subprocess.run([sys.executable, str(_SOURCE), "--root", str(self.root), "--change", self.change_id, "--json"], capture_output=True, text=True)
        self.assertEqual(1, result.returncode)
        self.assertFalse(json.loads(result.stdout)["valid"])


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        from product_execution import tree_digest
        from test_plan_contract import PLAN_PATH, SOURCE_FILES

        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        repository = Path(__file__).resolve().parents[2]
        for relative in [PLAN_PATH, *SOURCE_FILES,
                         "docs/architecture/engineering-modules.csv",
                         "docs/architecture/engineering-components.csv"]:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(repository / relative, target)
        self.plan = json.loads((self.root / PLAN_PATH).read_text())
        self.output = self.root / ".build/release-evidence"
        self.output.mkdir(parents=True)
        (self.output / "proof.log").write_text("independent observed fixture output for guard tests only\n")
        self.digest = hashlib.sha256((self.output / "proof.log").read_bytes()).hexdigest()
        self.artifact = hashlib.sha256(b"one exact package").hexdigest()
        self.registry = {"guard-source-fixture": (("guard-source-fixture",), "source_build"),
                         "guard-runtime-fixture": (("guard-runtime-fixture",), "final_package")}
        self.registry_patch = patch.object(contract, "_release_adapter_registry", return_value=self.registry)
        self.registry_patch.start()
        self.addCleanup(self.registry_patch.stop)
        source = tree_digest(self.root)
        self.receipts = {}
        self.document = {"schema_version": 2, "artifact_sha256": self.artifact,
                         "results": [{"case_id": "CR-01-INSTALL", "status": "passed", "artifact_sha256": self.artifact,
                                      "evidence": [{"path": ".build/release-evidence/proof.log", "sha256": self.digest}]}],
                         "execution_results": []}
        variants = {row["id"]: row for row in self.plan["variants"]}
        for scope, adapter_id in [("source_build", "guard-source-fixture"), ("final_package", "guard-runtime-fixture")]:
            receipt = {"schema_version": 1, "adapter_id": adapter_id, "command": list(self.registry[adapter_id][0]),
                       "status": "passed", "errors": [], "exit_code": 0,
                       "execution_nonce": "a" * 32 if scope == "source_build" else "b" * 32,
                       "source_tree_sha256": source, "artifact_sha256": self.artifact,
                       "product_component_tests_executed": scope == "source_build",
                       "product_packaged_native_game_tests_executed": scope == "final_package",
                       "fixture_substitution": False,
                       "started_at": "2026-10-19T00:00:00Z", "finished_at": "2026-10-19T00:00:02Z",
                       "execution_results": []}
            for case in self.plan["cases"]:
                if case["release_required"] is not True:
                    continue
                for variant_id in case["variant_ids"]:
                    variant = variants[variant_id]
                    if variant["evidence_scope"] != scope:
                        continue
                    identifier = f"{case['id']}@{variant_id}"
                    receipt["execution_results"].append({
                        "execution_id": identifier, "case_id": case["id"], "variant_id": variant_id,
                        "status": "passed", "evidence_scope": scope, "environment_ids": variant["environment_ids"],
                        "source_tree_sha256": source, "artifact_sha256": self.artifact,
                        "execution_nonce": receipt["execution_nonce"], "fixture_substitution": False,
                        "assertion_count": 2, "skipped_assertions": 0, "observed_at": "2026-10-19T00:00:01Z",
                        "evidence": [{"path": ".build/release-evidence/proof.log", "sha256": self.digest}],
                    })
                    self.document["execution_results"].append({"execution_id": identifier, "adapter_id": adapter_id})
            self.receipts[scope] = receipt
            self.save_receipt(scope)

    def save_receipt(self, scope, *, update_log=True):
        receipt = self.receipts[scope]
        log = self.output / f"{scope}.log"
        if update_log:
            log.write_text("\n".join("LUNA_RELEASE_RESULT " + json.dumps(row, sort_keys=True)
                                      for row in receipt["execution_results"]) + "\n")
        receipt["log"] = str(log.relative_to(self.root))
        receipt["log_sha256"] = hashlib.sha256(log.read_bytes()).hexdigest()
        path = self.output / f"{scope}.json"
        path.write_text(json.dumps(receipt))
        reference = {"path": str(path.relative_to(self.root)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
        for row in self.document["execution_results"]:
            if row["adapter_id"] == receipt["adapter_id"]:
                row["receipt"] = reference.copy()

    def errors(self, required=None):
        (self.output / "release.json").write_text(json.dumps(self.document))
        return contract.validate_release(self.root, ".build/release-evidence/release.json", self.artifact, required if required is not None else ["CR-01-INSTALL"])

    def test_same_artifact_real_evidence_and_complete_cases_pass(self):
        self.assertEqual([], self.errors())

    def test_fake_pass_without_evidence_is_rejected(self):
        self.document["results"][0]["evidence"] = []
        self.assertTrue(any("nonempty file/hash" in error for error in self.errors()))

    def test_empty_evidence_even_with_correct_hash_is_rejected(self):
        (self.output / "proof.log").write_bytes(b"")
        self.document["results"][0]["evidence"][0]["sha256"] = hashlib.sha256(b"").hexdigest()
        self.assertTrue(any("empty evidence" in error for error in self.errors()))

    def test_wrong_hash_and_missing_file_rejected(self):
        self.document["results"][0]["evidence"][0]["sha256"] = "0" * 64
        self.assertTrue(any("hash mismatch" in error for error in self.errors()))
        (self.output / "proof.log").unlink()
        self.assertTrue(any("missing or nonregular" in error for error in self.errors()))

    def test_evidence_from_another_package_is_rejected(self):
        self.document["results"][0]["artifact_sha256"] = "0" * 64
        self.assertTrue(any("another artifact" in error for error in self.errors()))

    def test_missing_duplicate_or_blocked_case_rejected(self):
        self.assertTrue(any("missing required case" in error for error in self.errors(["CR-01-INSTALL", "CR-03-SR"])))
        self.document["results"].append(dict(self.document["results"][0]))
        self.assertTrue(any("duplicate case" in error for error in self.errors()))
        self.document["results"][0]["status"] = "blocked"
        self.assertTrue(any("must be passed" in error for error in self.errors()))

    def test_escape_and_symlink_evidence_rejected(self):
        self.document["results"][0]["evidence"][0]["path"] = "../proof.log"
        self.assertTrue(any("traversal" in error for error in self.errors()))
        (self.output / "link.log").symlink_to(self.output / "proof.log")
        self.document["results"][0]["evidence"][0]["path"] = ".build/release-evidence/link.log"
        self.assertTrue(any("symbolic links" in error for error in self.errors()))

    def test_empty_required_case_configuration_fails_closed(self):
        self.assertTrue(any("required_case_ids" in error for error in self.errors([])))

    def test_flat_97_cases_cannot_hide_a_missing_execution_variant(self):
        from copy import deepcopy

        cases = [case["source"]["id"] for case in self.plan["cases"] if case["source"] is not None]
        self.assertEqual(97, len(cases))
        template = self.document["results"][0]
        self.document["results"] = [dict(deepcopy(template), case_id=identifier) for identifier in cases]
        self.assertEqual(153, len(self.document["execution_results"]))
        missing = self.document["execution_results"].pop()["execution_id"]
        errors = self.errors(cases)
        self.assertTrue(any(f"missing required planned combination ({missing})" in error for error in errors), errors)
        self.assertFalse(any("missing required case" in error for error in errors), errors)

    def test_missing_duplicate_and_unknown_execution_inventory_are_rejected(self):
        from copy import deepcopy

        original = deepcopy(self.document)
        for mutation, fragment in [("missing", "expected all planned"),
                                   ("duplicate", "duplicate planned combination"),
                                   ("unknown", "unknown planned combination")]:
            with self.subTest(mutation=mutation):
                self.document = deepcopy(original)
                if mutation == "missing":
                    del self.document["execution_results"]
                elif mutation == "duplicate":
                    self.document["execution_results"].append(deepcopy(self.document["execution_results"][0]))
                else:
                    self.document["execution_results"][0]["execution_id"] = "CR-01-INSTALL@MADE-UP"
                self.assertTrue(any(fragment in error for error in self.errors()))

    def test_component_adapter_cannot_substitute_for_native_game_execution(self):
        row = next(row for row in self.document["execution_results"] if row["adapter_id"] == "guard-runtime-fixture")
        row["adapter_id"] = "guard-source-fixture"
        self.assertTrue(any("requires a code-registered final_package" in error for error in self.errors()))

    def test_actual_closed_registry_without_native_adapter_rejects_fixture_adapter(self):
        self.registry_patch.stop()
        self.assertTrue(any("requires a code-registered final_package" in error for error in self.errors()))

    def test_no_runtime_and_fixture_substitution_are_rejected(self):
        for field, value in [("product_packaged_native_game_tests_executed", False), ("fixture_substitution", True)]:
            with self.subTest(field=field):
                receipt = self.receipts["final_package"]
                previous = receipt[field]
                receipt[field] = value
                self.save_receipt("final_package")
                self.assertTrue(any("fixture/no-runtime substitution" in error for error in self.errors()))
                receipt[field] = previous

    def test_wrong_source_package_command_or_nonce_are_rejected(self):
        receipt = self.receipts["final_package"]
        for field, value, fragment in [("source_tree_sha256", "0" * 64, "source tree differs"),
                                      ("artifact_sha256", "0" * 64, "another candidate artifact"),
                                      ("command", ["unregistered-tool"], "identity/command"),
                                      ("execution_nonce", "", "actual execution nonce")]:
            with self.subTest(field=field):
                original = receipt[field]
                receipt[field] = value
                self.save_receipt("final_package")
                self.assertTrue(any(fragment in error for error in self.errors()))
                receipt[field] = original

    def test_actual_execution_result_assertions_environment_and_timestamp_are_checked(self):
        from copy import deepcopy

        actual = self.receipts["final_package"]["execution_results"][0]
        original = deepcopy(actual)
        mutations = [("assertion_count", 0, "actual nonzero assertions"),
                     ("assertion_count", True, "actual nonzero assertions"),
                     ("skipped_assertions", 1, "zero skipped assertions"),
                     ("environment_ids", ["ENV-WEBVIEW"], "identity/environment"),
                     ("status", "not_run", "identity/environment"),
                     ("fixture_substitution", 0, "identity/environment"),
                     ("execution_nonce", "f" * 32, "identity/environment"),
                     ("observed_at", "2020-01-01T00:00:00Z", "timestamps must bind")]
        for field, value, fragment in mutations:
            with self.subTest(field=field, value=value):
                actual.clear()
                actual.update(deepcopy(original))
                actual[field] = value
                self.save_receipt("final_package")
                self.assertTrue(any(fragment in error for error in self.errors()))

    def test_changed_summary_cannot_override_actual_log_assertions(self):
        self.receipts["final_package"]["execution_results"][0]["assertion_count"] += 1
        self.save_receipt("final_package", update_log=False)
        self.assertTrue(any("differs from actual adapter log" in error for error in self.errors()))

    def test_empty_or_unstructured_log_cannot_back_passed_summary(self):
        (self.output / "final_package.log").write_text("all assertions passed; no actual per-case records\n")
        self.save_receipt("final_package", update_log=False)
        self.assertTrue(any("missing actual per-combination" in error for error in self.errors()))

    def test_mutated_runner_receipt_or_log_hash_is_rejected(self):
        (self.output / "final_package.json").write_text("{}")
        self.assertTrue(any("hash mismatch" in error for error in self.errors()))

    def test_runner_capture_command_only_accepts_bound_output_directory(self):
        from product_execution import PRODUCT_CAPTURE_CHECKS

        identifier = next(iter(PRODUCT_CAPTURE_CHECKS))
        command = tuple(PRODUCT_CAPTURE_CHECKS[identifier])
        receipt = self.receipts["source_build"]
        old_id = receipt["adapter_id"]
        receipt["adapter_id"] = identifier
        receipt["command"] = [*command, "--output", str((self.output / "capture").resolve())]
        self.registry[identifier] = command, "source_build"
        for row in self.document["execution_results"]:
            if row["adapter_id"] == old_id:
                row["adapter_id"] = identifier
        self.save_receipt("source_build")
        self.assertEqual([], self.errors())
        receipt["command"][-1] = str(self.root / "unrelated-output")
        self.save_receipt("source_build")
        self.assertTrue(any("identity/command" in error for error in self.errors()))

    def test_duplicate_malformed_and_non_json_log_records_are_rejected(self):
        log = self.output / "final_package.log"
        original = log.read_text()
        first = original.splitlines()[0]
        for content, fragment in [(original + first + "\n", "duplicate execution ID"),
                                  ('LUNA_RELEASE_RESULT {"execution_id":"x","execution_id":"y"}\n', "duplicate JSON key"),
                                  ('LUNA_RELEASE_RESULT {"execution_id":NaN}\n', "non-JSON constant"),
                                  ('LUNA_RELEASE_RESULT []\n', "empty or duplicate execution ID")]:
            with self.subTest(fragment=fragment):
                log.write_text(content)
                self.save_receipt("final_package", update_log=False)
                self.assertTrue(any(fragment in error for error in self.errors()))

    def test_duplicate_or_missing_actual_assertion_record_is_rejected(self):
        from copy import deepcopy

        receipt = self.receipts["final_package"]
        first = deepcopy(receipt["execution_results"][0])
        receipt["execution_results"].append(deepcopy(first))
        self.save_receipt("final_package")
        self.assertTrue(any("duplicate assertion result identities" in error for error in self.errors()))
        receipt["execution_results"] = [row for row in receipt["execution_results"] if row["execution_id"] != first["execution_id"]]
        self.save_receipt("final_package")
        self.assertTrue(any("missing unique actual assertions" in error for error in self.errors()))

    def test_observed_evidence_must_exist_be_unique_and_match_hash(self):
        from copy import deepcopy

        actual = self.receipts["final_package"]["execution_results"][0]
        original = deepcopy(actual["evidence"])
        for evidence, fragment in [([], "nonempty observed file/hash"),
                                   ([*original, *original], "duplicate observed evidence"),
                                   ([{"path": ".build/release-evidence/absent.png", "sha256": "a" * 64}], "missing or nonregular")]:
            with self.subTest(fragment=fragment):
                actual["evidence"] = evidence
                self.save_receipt("final_package")
                self.assertTrue(any(fragment in error for error in self.errors()))

    def test_failed_runner_boolean_exit_and_malformed_receipts_are_rejected(self):
        receipt = self.receipts["final_package"]
        for field, value, fragment in [("schema_version", True, "schema_version integer 1"),
                                      ("status", "failed", "exit zero with no errors"),
                                      ("exit_code", False, "exit zero with no errors"),
                                      ("errors", ["real failure"], "exit zero with no errors"),
                                      ("execution_results", [None], "malformed or duplicate"),
                                      ("started_at", "2026-10-19T00:00:00", "timestamps must bind")]:
            with self.subTest(field=field):
                previous = receipt[field]
                receipt[field] = value
                self.save_receipt("final_package")
                self.assertTrue(any(fragment in error for error in self.errors()))
                receipt[field] = previous

    def test_changed_source_after_execution_requires_rerun(self):
        (self.root / "new-source.rs").write_text("fn changed_source() {}\n")
        self.assertTrue(any("source tree differs" in error for error in self.errors()))

    def test_invalid_plan_cannot_shrink_required_execution_inventory(self):
        path = self.root / "docs/development/first-product-test-plan.json"
        self.plan["cases"] = self.plan["cases"][:1]
        path.write_text(json.dumps(self.plan))
        self.assertTrue(any("release test plan:" in error for error in self.errors()))

    def test_legacy_release_schema_is_not_complete_execution_evidence(self):
        self.document["schema_version"] = 1
        self.assertTrue(any("expected integer 2" in error for error in self.errors()))

    def test_stability_runtime_registration_is_exact_and_not_full_release_coverage(self):
        from product_execution import RUNTIME_STABILITY_CHECK, PRODUCT_RUNTIME_CHECKS
        from spec_contract import _release_adapter_registry, _registered_release_command
        registration=_release_adapter_registry()[RUNTIME_STABILITY_CHECK]
        self.assertEqual(registration,(tuple(PRODUCT_RUNTIME_CHECKS[RUNTIME_STABILITY_CHECK]),'final_package'))
        receipt=self.root/'runtime.json'
        receipt.write_text(json.dumps({'execution_nonce':'a'*32}))
        command=_registered_release_command(RUNTIME_STABILITY_CHECK,registration,receipt)
        self.assertEqual(command,list(PRODUCT_RUNTIME_CHECKS[RUNTIME_STABILITY_CHECK])+['--inputs',str(receipt.parent/'runtime-inputs.json'),'--output',str(receipt.parent/'runtime'),'--execution-nonce','a'*32])
        receipt.write_text(json.dumps({'execution_nonce':'not-a-launcher-nonce'}))
        self.assertEqual(_registered_release_command(RUNTIME_STABILITY_CHECK,registration,receipt),[])
        # The existing release inventory/actual per-combination assertions are
        # still mandatory; merely adding a runtime registry cannot satisfy them.
        self.document['execution_results']=[]
        self.assertTrue(self.errors())


if __name__ == "__main__":
    unittest.main()
