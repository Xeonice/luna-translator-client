"""Inspect original binding execution bytes and command/source provenance.

This does not believe a component's passed flag and does not create design or
Native acceptance. Its semantic oracle and retained-DOM recomputation are
separate from the production component runner.
"""
from __future__ import annotations

from datetime import datetime
import importlib.util
import json
from pathlib import Path
import re
import struct
import sqlite3
import sys
import tomllib
import zlib

from binding_phase_guard import (checked_ref, load, path_at, products, ref, sha,
                                 source_map, tree_sha)
from gate_core import source_files
from binding_journey import (COMPONENT_CASES, EXPORTER, RUNNER, PLAN,
                             NONCE, RUST_LAYERS, TRANSPORT_PRODUCER, NATIVE_STEPS, COMPONENT_SCENES,
                             SCENE_FALLBACKS, required_scene_methods, rust_cases, observation_verdict, scene_verdict)
from binding_journey import actual_scene_inputs, DIAGNOSTIC_SCENES, DIAGNOSTIC_METHODS
from binding_journey import FIXED_NATIVE_UNIT_GROUPS, EXACT_EXECUTION_OBLIGATION_IDS
from binding_journey import scoped_source_discharge


def interval(start, finish):
    a, b = datetime.fromisoformat(start), datetime.fromisoformat(finish)
    if a.tzinfo is None or b.tzinfo is None or b < a:
        raise ValueError('Timezone-aware ordered actual execution interval required')
    return a, b


def exact_test_outcome(raw, name, exit_code):
    """Read libtest's actual result, including --nocapture inline output."""
    summaries = list(re.finditer(r'^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', raw, re.M))
    names = list(re.finditer(r'^test ' + re.escape(name) + r' \.\.\. ([^\r\n]*)', raw, re.M))
    if len(summaries) != 1 or len(names) != 1 or type(exit_code) is not int or exit_code not in (0, 101):
        raise ValueError('One exact actual Rust test result is required')
    summary, marker = summaries[0], names[0]
    state, passed, failed, ignored = summary.groups()
    expected = 'ok' if exit_code == 0 else 'FAILED'
    if ((state, int(passed), int(failed), int(ignored)) !=
            (expected, 1 if exit_code == 0 else 0, 0 if exit_code == 0 else 1, 0)
            or marker.start() >= summary.start()):
        raise ValueError('Rust test exit/result/counts contradict the actual single case')
    # In serial mode libtest writes the test name before the test's stdout.
    # The final standalone result is still emitted by libtest before summary.
    terminal = marker.group(1).strip()
    if terminal != expected:
        between = raw[marker.end():summary.start()].strip().splitlines()
        if not between or between[-1].strip() != expected:
            raise ValueError('Actual named Rust test terminal result is missing')
    return exit_code == 0


def rpc_protocol_payload(line):
    prefix = 'LUNA_BINDING_RPC_V1 '
    harness = 'test bj_component_transport ... '
    if line.startswith(harness + prefix):
        line = line[len(harness):]
    return line[len(prefix):] if line.startswith(prefix) else None


def within(root, reference, directory, *, limit=16 * 1024 * 1024):
    p = checked_ref(root, reference, ignored=True)
    if not p.is_relative_to(Path(directory).resolve()) or not 0 < p.stat().st_size <= limit:
        raise ValueError('Evidence must belong to the actual invocation directory')
    return p


def png_dimensions(raw):
    if len(raw) < 33 or raw[:8] != b'\x89PNG\r\n\x1a\n' or raw[12:16] != b'IHDR':
        raise ValueError('Actual PNG bytes required')
    width, height = struct.unpack('>II', raw[16:24])
    if not (1 <= width <= 16384 and 1 <= height <= 16384):
        raise ValueError('PNG dimensions outside bounded image capacity')
    position, seen_idat, ended = 8, False, False
    compressed = bytearray()
    while position + 12 <= len(raw):
        size = struct.unpack('>I', raw[position:position + 4])[0]
        name = raw[position + 4:position + 8]
        finish = position + 12 + size
        if finish > len(raw) or size > 32 * 1024 * 1024:
            raise ValueError('Truncated/oversized PNG chunk')
        payload = raw[position + 8:position + 8 + size]
        crc = struct.unpack('>I', raw[position + 8 + size:finish])[0]
        if zlib.crc32(name + payload) != crc:
            raise ValueError('PNG chunk CRC differs')
        if name == b'IDAT':
            seen_idat = True
            compressed.extend(payload)
        position = finish
        if name == b'IEND':
            ended = True
            break
    if not seen_idat or not ended or position != len(raw):
        raise ValueError('Complete actual PNG stream required')
    decoder = zlib.decompressobj()
    decoded = decoder.decompress(bytes(compressed), min(256 * 1024 * 1024, width * height * 8 + height + 65536))
    if not decoded or not decoder.eof or decoder.unconsumed_tail:
        raise ValueError('PNG raster does not decode within image bounds')
    return width, height


def inspect_command(root, reference, directory, *, expected_code=0):
    record = load(within(root, reference, directory))
    if (not isinstance(record.get('command'), list) or not record['command']
            or any(type(x) is not str for x in record['command'])
            or type(record.get('exit_code')) is not int
            or record['exit_code'] != expected_code):
        raise ValueError('Actual expected command/exit receipt required')
    interval(record['started_at'], record['finished_at'])
    for key in ('stdout', 'stderr'):
        p = checked_ref(root, record[key], ignored=True)
        if not p.is_relative_to(directory):
            raise ValueError('Command log does not belong to producer')
    cwd = path_at(root, record['cwd']) if record['cwd'] != '.' else Path(root)
    if not cwd.is_dir():
        raise ValueError('Recorded command working directory missing')
    return record


def lock_registry(lock):
    rows = tomllib.loads(lock.read_text()).get('package', [])
    result = {}
    for row in rows:
        if row.get('source', '').startswith('registry+'):
            key = (row['name'], row['version'], row['source'])
            if key in result:
                raise ValueError('Duplicate Cargo registry package identity')
            result[key] = row.get('checksum')
    return result


def cargo_package_id(root, package):
    path = Path(root).resolve() / 'crates' / package
    document = tomllib.loads((path / 'Cargo.toml').read_text())
    version = document['package']['version']
    if isinstance(version, dict):
        version = tomllib.loads((Path(root) / 'Cargo.toml').read_text())['workspace']['package']['version']
    return 'path+' + path.as_uri() + '#' + version


def inspect_test_writer(root, proof, directory, spec, source):
    binary = within(root, proof['binary'], directory, limit=256 * 1024 * 1024)
    compilation = inspect_command(root, proof['compilation'], directory)
    expected = ['cargo', 'test', '--locked', '--offline', '-p', spec['package'], '--test', spec['target'], '--no-run', '--message-format=json']
    if compilation['command'] != expected or compilation['cwd'] != '.':
        raise ValueError('Exact registered production writer compilation required')
    artifacts = []
    for line in checked_ref(root, compilation['stdout'], ignored=True).read_text().splitlines():
        try:
            value = strict_json_text(line)
        except ValueError:
            continue
        if (value.get('reason') == 'compiler-artifact' and value.get('target', {}).get('name') == spec['target']
                and value.get('profile', {}).get('test') is True and value.get('executable')):
            artifacts.append(value)
    if (len(artifacts) != 1 or artifacts[0].get('package_id') != cargo_package_id(root, spec['package'])
            or Path(artifacts[0]['target']['src_path']).resolve() != Path(root).resolve() / spec['source']
            or proof.get('producer_source') not in (None, {'path': spec['source'], 'sha256': source.get(spec['source'])})):
        raise ValueError('Actual compiled writer package/source/binary identity differs')
    inventory = inspect_command(root, proof['inventory'], directory)
    if (inventory['command'] != [str(binary), '--list', '--ignored', '--format', 'terse']
            or checked_ref(root, inventory['stdout'], ignored=True).read_text().splitlines().count(spec['test_name'] + ': test') != 1):
        raise ValueError('Exact compiled ignored writer inventory missing')
    execution = inspect_command(root, proof['execution'], directory)
    if (execution['command'] != [str(binary), '--ignored', '--exact', spec['test_name'], '--nocapture']
            or execution['cwd'] != '.'):
        raise ValueError('Actual exact registered test writer command missing')
    raw = checked_ref(root, execution['stdout'], ignored=True).read_text()
    if not exact_test_outcome(raw, spec['test_name'], execution['exit_code']):
        raise ValueError('Registered test writer did not actually execute exactly once')
    return execution


def pointer(document, value):
    if not isinstance(value, str) or not value.startswith('/'):
        raise ValueError('Exact actual serialized response JSON pointer required')
    for part in value.split('/')[1:]:
        part = part.replace('~1', '/').replace('~0', '~')
        document = document[int(part)] if isinstance(document, list) else document[part]
    return document


def inspect_transport(root, transport, nonce, expected_sources):
    """Consume a compiled, exact production Rust writer, not caller JSON flags."""
    spec = TRANSPORT_PRODUCER
    if (transport.get('kind') != 'binding-journey-runtime-transport/v1'
            or transport.get('nonce') != nonce or transport.get('source_tree_sha256') != tree_sha(expected_sources)
            or transport.get('producer') != dict(spec, source_sha256=expected_sources.get(spec['source']))
            or spec['source'] not in expected_sources):
        raise ValueError('Registered actual typed transport producer identity differs')
    binary_path = checked_ref(root, transport['binary'], ignored=True)
    directory = binary_path.parent
    binary = within(root, transport['binary'], directory, limit=256 * 1024 * 1024)
    compile_record = inspect_command(root, transport['compilation'], directory)
    if compile_record['command'] != ['cargo', 'test', '--locked', '--offline', '-p', spec['package'],
                                     '--test', spec['target'], '--no-run', '--message-format=json'] or compile_record['cwd'] != '.':
        raise ValueError('Exact actual transport target compilation required')
    artifacts = []
    for line in checked_ref(root, compile_record['stdout'], ignored=True).read_text().splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if value.get('reason') == 'compiler-artifact' and value.get('target', {}).get('name') == spec['target'] and value.get('profile', {}).get('test'):
            artifacts.append(value)
    if (len(artifacts) != 1 or not artifacts[0].get('executable')
            or artifacts[0].get('package_id') != cargo_package_id(root, spec['package'])
            or Path(artifacts[0]['target']['src_path']).resolve() != Path(root).resolve() / spec['source']):
        raise ValueError('Transport Cargo artifact is not the registered production test source')
    inventory = inspect_command(root, transport['inventory'], directory)
    if inventory['command'] != [str(binary), '--list', '--ignored', '--format', 'terse']:
        raise ValueError('Registered ignored transport test inventory required')
    names = re.findall(r'^([\w:]+): test$', checked_ref(root, inventory['stdout'], ignored=True).read_text(), re.M)
    if names.count(spec['test_name']) != 1:
        raise ValueError('One actual registered typed transport writer required')
    if transport.get('mode') == 'ndjson':
        if (transport.get('protocol') != 'LUNA_BINDING_RPC_V1'
                or any(field in transport for field in ('file', 'execution'))):
            raise ValueError('Dynamic transport cannot supply prewritten DTOs/execution')
        return {'mode': 'ndjson', 'producer': transport}
    raise ValueError('Only actual runtime NDJSON transport is registered; static response files cannot fill journeys')


def inspect_producer(root, reference, nonce, expected_sources):
    manifest_path = checked_ref(root, reference, ignored=True)
    directory = manifest_path.parent
    manifest = load(manifest_path)
    if (type(manifest.get('schema_version')) is not int or manifest['schema_version'] != 1
            or manifest.get('kind') != 'binding-journey-production-projections/v1'
            or manifest.get('nonce') != nonce or manifest.get('status') != 'passed'
            or manifest.get('source_stable') is not True
            or manifest.get('infrastructure_errors') != []
            or manifest.get('native_executed') is not False
            or manifest.get('product_acceptance_promoted') is not False
            or manifest.get('source_before') != products(expected_sources)
            or manifest.get('source_after') != products(expected_sources)):
        raise ValueError('Actual production Rust projection manifest identity differs')
    begin, end = interval(manifest['started_at'], manifest['finished_at'])
    closure = source_map(manifest['sources'])
    required = {'Cargo.toml', 'Cargo.lock', 'crates/app-core/Cargo.toml',
                'apps/desktop/src-tauri/src/platform/window_preview/state.rs'}
    required.update(p for p in expected_sources if p.startswith('crates/app-core/src/'))
    if set(closure) != required or any(expected_sources.get(p) != h for p, h in closure.items()):
        raise ValueError('Production Rust closure differs from actual execution source')
    if manifest.get('producer') != {'path': EXPORTER, 'sha256': expected_sources.get(EXPORTER)}:
        raise ValueError('Projection exporter source identity differs')
    source_path = within(root, manifest['generated_rust'], directory)
    cargo_path = within(root, manifest['generated_cargo'], directory)
    if source_path.name != 'main.rs' or cargo_path.name != 'Cargo.toml':
        raise ValueError('Exact isolated Rust source/package inputs required')
    exporter_path = path_at(root, EXPORTER)
    if sha(exporter_path.read_bytes()) != expected_sources.get(EXPORTER):
        raise ValueError('Code-owned exporter changed since producer execution')
    spec = importlib.util.spec_from_file_location('binding_projection_exporter_inspection', exporter_path)
    exporter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(exporter)
    if (source_path.read_text() != exporter.rust_source_text(Path(root).resolve())
            or cargo_path.read_text() != exporter.cargo_manifest_text(Path(root).resolve())):
        raise ValueError('Exact code-owned production Rust/Cargo generation inputs differ')
    binary = within(root, manifest['binary'], directory, limit=256 * 1024 * 1024)
    if binary.name != 'binding-journey-projection-producer':
        raise ValueError('Exact compiled projection producer required')
    # An isolated resolver must use exactly the root-locked registry versions
    # and checksums, rather than independently resolving serde or transitive deps.
    cargo_lock_ref = manifest.get('generated_lock') or manifest.get('cargo_lock')
    if cargo_lock_ref is None:
        raise ValueError('Isolated Cargo.lock proof missing')
    cargo_lock = within(root, cargo_lock_ref, directory)
    root_lock = path_at(root, 'Cargo.lock')
    if sha(root_lock.read_bytes()) != expected_sources['Cargo.lock']:
        raise ValueError('Current root lock differs from producer context')
    isolated_packages, root_packages = lock_registry(cargo_lock), lock_registry(root_lock)
    if not isolated_packages or any(root_packages.get(k) != v for k, v in isolated_packages.items()):
        raise ValueError('Isolated Rust dependencies/checksums differ from root lock')
    executions = manifest.get('executions')
    if not isinstance(executions, list) or len(executions) != 3:
        raise ValueError('Actual lock/compile/run receipts are all required')
    commands = [inspect_command(root, value, directory) for value in executions]
    lock_command, compile_command, run_command = [v['command'] for v in commands]
    if (lock_command != ['cargo', 'metadata', '--offline', '--format-version', '1']
            or compile_command != ['cargo', 'build', '--locked', '--offline', '--quiet']
            or run_command != [str(binary), nonce]
            or manifest.get('command_execution') != executions[2]):
        raise ValueError('Projection commands differ from code-owned actual registry')
    expected_cwd = cargo_path.parent.relative_to(root).as_posix()
    if any(command['cwd'] != expected_cwd for command in commands):
        raise ValueError('Projection commands must use the exact generated Cargo project')
    previous = begin
    for command in commands:
        a, b = interval(command['started_at'], command['finished_at'])
        if not previous <= a <= b <= end:
            raise ValueError('Projection commands lie outside ordered producer interval')
        previous = b
    projection_file = within(root, manifest['projection_file'], directory)
    values = load(projection_file)
    raw = load(checked_ref(root, commands[-1]['stdout'], ignored=True))
    if (values != raw or type(values.get('schema_version')) is not int or values['schema_version'] != 1
            or values.get('kind') != 'binding-journey-production-projection-values/v1'
            or values.get('nonce') != nonce
            or values.get('selection_required', {}).get('errorCode') != 'preview_window_selection_required'):
        raise ValueError('Projection JSON differs from actual Rust serialized stdout')
    runtime_transport = manifest.get('runtime_transport')
    runtime_values = None
    if runtime_transport is not None:
        runtime_values = inspect_transport(root, runtime_transport, nonce, expected_sources)
        original = load(checked_ref(root, manifest['runtime_transport_manifest'], ignored=True))
        if original != runtime_transport:
            raise ValueError('Merged runtime transport differs from original actual writer envelope')
    return {'manifest': manifest, 'values': values, 'binary': binary,
            'projection_file': projection_file, 'commands': commands, 'runtime_values': runtime_values}


def strict_json_text(raw):
    from binding_phase_guard import unique
    return json.loads(raw, object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('Invalid RPC JSON value')))


def inspect_rpc(root, report, directory, nonce, source, producer):
    transport = producer['manifest'].get('runtime_transport')
    if transport is None:
        if report.get('transport_execution') is not None or report.get('transport_ready') is not None:
            raise ValueError('RPC execution claimed without an actual registered production writer')
        return {'ready': None, 'messages': []}
    binary = checked_ref(root, transport['binary'], ignored=True)
    execution = inspect_command(root, report['transport_execution'], directory)
    if (execution['command'] != [str(binary), '--ignored', '--exact', 'bj_component_transport', '--nocapture']
            or execution['cwd'] != '.' or execution.get('environment') != {
                'LUNA_BINDING_NONCE': nonce, 'LUNA_BINDING_SOURCE_TREE': tree_sha(source), 'LUNA_BINDING_MODE': 'stdio',
                'LUNA_BINDING_OUTPUT': str(directory / 'rpc-profiles')}
            or type(execution.get('pid')) is not int or execution['pid'] < 1
            or execution.get('protocol_errors') != [] or execution.get('signal') is not None):
        raise ValueError('Actual dynamic Rust fixture command/environment/process differs')
    transcript = load(within(root, execution['transcript'], directory))
    if (transcript.get('kind') != 'binding-journey-runtime-rpc-transcript/v1'
            or transcript.get('schema_version') != 1 or transcript.get('nonce') != nonce):
        raise ValueError('Original actual RPC transcript identity differs')
    prefix = 'LUNA_BINDING_RPC_V1 '
    inputs = checked_ref(root, execution['stdin'], ignored=True).read_text()
    output = checked_ref(root, execution['stdout'], ignored=True).read_text()
    if not exact_test_outcome(output, 'bj_component_transport', execution['exit_code']):
        raise ValueError('Actual dynamic production writer test did not execute exactly once')
    requests = []
    for line in inputs.splitlines():
        if not line.startswith(prefix):
            raise ValueError('Unregistered RPC request framing')
        requests.append(strict_json_text(line[len(prefix):]))
    messages = [strict_json_text(payload) for line in output.splitlines()
                if (payload := rpc_protocol_payload(line)) is not None]
    ready = [row for row in messages if row.get('type') == 'ready']
    if not ready:
        if requests or transcript.get('messages') or transcript.get('ready') is not None or report.get('transport_ready') is not None:
            raise ValueError('Unavailable RPC writer cannot invent requests/results')
        return {'ready': None, 'messages': []}
    if len(ready) != 1 or messages[0] != ready[0]:
        raise ValueError('Actual RPC ready must appear exactly once before responses')
    ready = ready[0]
    if (ready != transcript.get('ready') or ready != report.get('transport_ready')
            or set(ready) != {'type', 'schema_version', 'nonce', 'source_tree_sha256', 'capabilities'}
            or ready['schema_version'] != 1 or ready['nonce'] != nonce or ready['source_tree_sha256'] != tree_sha(source)):
        raise ValueError('Actual RPC ready/source identity differs')
    capabilities = ready['capabilities']
    if set(capabilities) != {'scene_ids', 'methods', 'controls'}:
        raise ValueError('Exact RPC capability fields required')
    interface = path_at(root, 'apps/desktop/src/bridge/application-port.ts').read_text()
    actual_methods = set(re.findall(r'^\s+([A-Za-z][A-Za-z0-9]*)\(', interface, re.M)) | {
        'reviewAssociation', 'associationProgress', 'sourceSelection', 'beginReading'}
    for key, allowed in [('scene_ids', set(COMPONENT_SCENES)), ('methods', actual_methods), ('controls', {'reset', 'deliver'})]:
        values = capabilities[key]
        if not isinstance(values, list) or any(type(value) is not str or value not in allowed for value in values) or len(values) != len(set(values)):
            raise ValueError('RPC capability is not code-registered: ' + key)
    responses = messages[1:]
    if len(responses) != len(requests) or len(transcript.get('messages', [])) != len(requests):
        raise ValueError('Actual RPC has dropped/extra/unconsumed requests or responses')
    for number, request in enumerate(requests, 1):
        if (set(request) != {'type', 'schema_version', 'nonce', 'id', 'scenario', 'action', 'method', 'args'}
                or request['type'] != 'request' or request['schema_version'] != 1 or request['nonce'] != nonce
                or request['id'] != str(number) or request['scenario'] not in capabilities['scene_ids']
                or request['action'] not in {'invoke', 'reset', 'deliver'} or not isinstance(request['args'], list)):
            raise ValueError('Actual RPC request ID/scenario/schema differs')
        if request['action'] == 'invoke':
            if request['method'] not in capabilities['methods']:
                raise ValueError('UI invoked an unregistered actual Rust application method')
        elif request['method'] is not None or request['action'] not in capabilities['controls']:
            raise ValueError('RPC control was substituted for an application call')
    by_id = {request['id']: request for request in requests}
    seen = set()
    for response, claimed in zip(responses, transcript['messages']):
        response_id = response.get('id')
        if response_id not in by_id or response_id in seen:
            raise ValueError('RPC response ID is unknown or duplicated')
        seen.add(response_id)
        request = by_id[response_id]
        if (set(response) != {'type', 'schema_version', 'nonce', 'id', 'scenario', 'action', 'method', 'outcome', 'response', 'error', 'publications', 'previewUpdates', 'events'}
                or response['type'] != 'response' or response['schema_version'] != 1 or response['nonce'] != nonce
                or any(response[key] != request[key] for key in ('id', 'scenario', 'action', 'method'))
                or response['outcome'] not in {'ok', 'error'}
                or any(not isinstance(response[key], list) for key in ('publications', 'previewUpdates', 'events'))
                or (response['error'] is not None if response['outcome'] == 'ok' else response['response'] is not None)
                or claimed != {'request': request, 'response': response}):
            raise ValueError('Actual RPC response correlation/typed opaque publication differs')
    return {**transcript, 'inspected_execution': execution}


def inspect_source_details(step, rpc):
    """Rebuild detail obligations from actual response entity ownership."""
    details = step.get('source_details')
    if not isinstance(details, dict):
        raise ValueError('Actual per-source detail observations missing')
    replies = step['facts']['rpcMessages']
    reset = next((row for row in replies if row.get('action') == 'reset' and row.get('outcome') == 'ok'), None)
    metadata = reset.get('response', {}).get('source_detail_obligations', []) if reset else []
    if details.get('metadata') != metadata or not isinstance(metadata, list):
        raise ValueError('Source detail metadata was not part of the actual production reset response')
    if len(details.get('rows', [])) != len(metadata):
        raise ValueError('Source detail observations omitted a production source')
    seen, advanced, precision = set(), bool(metadata), False
    for obligation, observed in zip(metadata, details['rows']):
        identity = obligation.get('source_id')
        source_pointer = obligation.get('source_pointer')
        id_pointer = obligation.get('source_id_pointer')
        if (type(identity) is not str or not identity or identity in seen
                or type(source_pointer) is not str or not source_pointer.startswith('/')
                or type(id_pointer) is not str or len(id_pointer.split('/')) != 2 or not id_pointer.startswith('/')):
            raise ValueError('Detail must identify one actual source entity and its direct scalar ID')
        seen.add(identity)
        origin = next((row for row in replies if row.get('id') == obligation['response_id']), None) if obligation.get('response_id') else reset
        if origin is None:
            raise ValueError('Source detail value has no actual production RPC response')
        root_value = origin['response'] if obligation.get('response_id') else origin['response']['initial_snapshot']
        source_object = pointer(root_value, source_pointer)
        if not isinstance(source_object, dict) or pointer(source_object, id_pointer) != identity:
            raise ValueError('Detail fields are not owned by the actual source ID entity')
        if (observed.get('source_id') != identity or observed.get('source_pointer') != source_pointer
                or observed.get('source_id_pointer') != id_pointer or observed.get('response_id') != origin['id']
                or len(observed.get('fields', [])) != len(obligation['fields'])):
            raise ValueError('Observed source detail identity differs from actual source object')
        labels = set()
        for field, actual in zip(obligation['fields'], observed['fields']):
            label, field_pointer = field.get('label'), field.get('json_pointer')
            if label not in {'Hook', 'Address', 'Context', 'Context 2'} or label in labels or not isinstance(field_pointer, str) or not field_pointer.startswith(source_pointer + '/'):
                raise ValueError('Source detail field escaped its exact source entity subtree')
            labels.add(label)
            value = pointer(root_value, field_pointer)
            if value != field.get('value') or any(actual.get(k) != v for k, v in field.items()) or actual.get('expected') != value or actual.get('responseId') != origin['id'] or actual.get('inputAvailable') is not (type(value) is str):
                raise ValueError('Observed source expected value was rewritten or came from another response')
            if type(value) is not str:
                advanced = False
            elif label != 'Hook':
                if not re.fullmatch(r'0|[1-9][0-9]{0,19}', value) or int(value) > 2 ** 64 - 1:
                    raise ValueError('Actual source numeric DTO violates canonical exact u64 transport')
                precision = precision or int(value) > 2 ** 53 - 1
        advanced = advanced and labels == {'Hook', 'Address', 'Context', 'Context 2'}
    blocked = ([] if advanced else ['advanced-source-typed-DTO-disclosure']) + ([] if precision else ['source-u64-typed-DTO-projection'])
    if (details.get('advancedAvailable') is not advanced or details.get('precisionAvailable') is not precision
            or details.get('blocked_obligations') != blocked):
        raise ValueError('Typed source detail/boundary coverage was declared without actual source inputs')
    return blocked


def inspect_library_owner_database(root, identity, steps, rpc, directory, nonce, source):
    """Rebuild seeded owners and no implicit SaveGames from sealed SQLite."""
    replies = steps[-1]['facts']['rpcMessages']
    reset = next(row['response'] for row in replies if row['action'] == 'reset')
    after_reply = next((row['response'] for row in reversed(replies) if row['action'] == 'deliver'
                        and isinstance(row.get('response'), dict) and 'sealed_database_after' in row['response']), {})
    views = []
    execution = rpc.get('inspected_execution') or {}
    started, finished = interval(execution['started_at'], execution['finished_at'])
    for reference in (reset.get('sealed_database_before'), after_reply.get('sealed_database_after')):
        manifest = load(within(root, reference, directory / 'rpc-profiles'))
        if (manifest.get('kind') != 'binding-library-database-snapshot/v1' or type(manifest.get('schema_version')) is not int or manifest.get('schema_version') != 1
                or manifest.get('nonce') != nonce or manifest.get('scene_id') != identity
                or manifest.get('source_tree_sha256') != tree_sha(source)
                or type(manifest.get('captured_at_ms')) is not int
                or not int(started.timestamp()*1000) <= manifest['captured_at_ms'] <= int(finished.timestamp()*1000)
                or type(manifest.get('pid')) is not int or manifest['pid'] != execution['pid']):
            raise ValueError('Actual library SQLite snapshot identity/clock differs')
        database = within(root, manifest['database'], directory / 'rpc-profiles', limit=128 * 1024 * 1024)
        if any(Path(str(database) + suffix).exists() for suffix in ('-wal', '-shm', '-journal')):
            raise ValueError('Library snapshot must be a sealed independent SQLite copy')
        connection = sqlite3.connect(database.as_uri() + '?mode=ro&immutable=1', uri=True)
        connection.row_factory = sqlite3.Row
        try:
            if connection.execute('pragma integrity_check').fetchone()[0] != 'ok' or connection.execute('pragma foreign_key_check').fetchall():
                raise ValueError('Actual library database integrity/foreign keys failed')
            view = {table: [dict(row) for row in connection.execute('select * from ' + table + ' order by 1')]
                    for table in ('games', 'game_targets', 'operation_receipts', 'profile_meta', 'schema_migrations')}
        finally:
            connection.close()
        profile = reset['initial_snapshot']
        meta = view['profile_meta']
        if (len(meta) != 1 or meta[0]['profile_id'] != profile['profileId']
                or meta[0]['server_instance_id'] != profile['serverInstanceId']
                or meta[0]['runtime_generation'] != profile['profileGeneration']):
            raise ValueError('Library SQLite snapshot belongs to another actual profile/generation')
        views.append((manifest, view))
    if (views[0][0]['captured_at_ms'] >= views[1][0]['captured_at_ms']
            or any(views[0][1][table] != views[1][1][table] for table in ('games', 'game_targets', 'schema_migrations'))):
        raise ValueError('Library journey implicitly edited games or rewrote snapshot time/ledger')
    before, after = views[0][1], views[1][1]
    snapshot = reset['initial_snapshot']
    for game in snapshot['games']:
        owners = [row for row in before['game_targets'] if row['game_id'] == game['gameId']
                  and tuple(row[field] for field in ('environment_key', 'canonical_exe_path', 'architecture'))
                  == tuple(game['target'][field] for field in ('environmentKey', 'canonicalExePath', 'architecture'))]
        if len(owners) != 1 or sum(row['game_id'] == game['gameId'] for row in before['games']) != 1:
            raise ValueError('Initial AppSnapshot game owner does not join actual SQLite full target identity')
    for seed in reset['seed_commands']:
        operation = seed['request']['operationId']
        rows = [row for row in before['operation_receipts'] if row['operation_id'] == operation]
        if (len(rows) != 1 or rows[0]['command_kind'] != 'save_games'
                or json.loads(rows[0]['result_refs_json']) != seed['response']
                or seed['response']['requestId'] != operation):
            raise ValueError('SaveGames seed receipt is not the actual SQLite atomic result')
    old = {row['operation_id'] for row in before['operation_receipts']}
    if any(row['operation_id'] not in old and row['command_kind'] in {'save_game', 'save_games'} for row in after['operation_receipts']):
        raise ValueError('Post-seed actual SQLite contains an implicit game-save mutation')
    actual_calls = [row['request'] for row in rpc['messages'] if row['request']['scenario'] == identity
                    and row['request']['action'] == 'invoke' and row['request']['method'] in {'reviewAssociation', 'associate', 'dispatch'}]
    observed_calls = [row for row in steps[-1]['facts']['methodCalls'] if row['method'] in {'reviewAssociation', 'associate', 'dispatch'}]
    if [(row['method'], row['args']) for row in actual_calls] != [(row['method'], row['args']) for row in observed_calls]:
        raise ValueError('Library review/scope/association request args differ from the actual RPC transcript')
    return {'status': 'passed', 'database_before': views[0][0]['database'], 'database_after': views[1][0]['database']}


def inspect_post_checkpoint_facts(checkpoint, post, actual_replies=None):
    if not isinstance(post, dict):
        raise ValueError('Explicit original post-checkpoint facts required; no late event can be dropped')
    for key in ('events', 'rpcMessages', 'methodCalls', 'transportCalls'):
        original, observed = checkpoint.get(key), post.get(key)
        if (not isinstance(original, list) or not isinstance(observed, list)
                or json.dumps(observed[:len(original)], sort_keys=True) != json.dumps(original, sort_keys=True)):
            raise ValueError('Post-checkpoint facts rewrote/dropped original prefix: ' + key)
    if actual_replies is not None:
        replies = post['rpcMessages']
        if (len({row.get('id') for row in replies}) != len(replies)
                or any(row.get('id') not in actual_replies or json.dumps(actual_replies[row['id']]['response'], sort_keys=True) != json.dumps(row, sort_keys=True) for row in replies)):
            raise ValueError('Late checkpoint RPC facts differ from the complete actual transport')
        effects = [event for reply in replies for event in reply['events']]
        if json.dumps(effects, sort_keys=True) != json.dumps([row['event'] for row in post['events'] if row.get('kind') == 'production_side_effect'], sort_keys=True):
            raise ValueError('Late checkpoint side effects were dropped/rewritten')


def inspect_finite_picker(observations, events, actual_rpc=None):
    """Bind the finite picker count to real port entries and original RPCs."""
    before, after = observations.get('beforeFacts'), observations.get('afterFacts')
    if not isinstance(before, dict) or not isinstance(after, dict):
        raise ValueError('Picker observation requires actual before/after facts')
    equal = lambda a, b: json.dumps(a, sort_keys=True) == json.dumps(b, sort_keys=True)
    if not equal(after.get('events'), events):
        raise ValueError('Picker final events differ from actual after facts')
    messages = actual_rpc.get('messages', []) if actual_rpc is not None else None
    actual = ({row['response']['id']: row for row in messages}
              if messages is not None else None)
    inspect_post_checkpoint_facts(before, after,
                                  {key: row for key, row in actual.items()} if actual is not None else None)
    for facts in (before, after):
        calls = [row for row in facts['events'] if row.get('kind') == 'port_call']
        if not equal(calls, facts.get('methodCalls')):
            raise ValueError('Picker port call inventory differs from original events')
        picks = [row for row in calls if row.get('method') == 'selectWindowPreview']
        if not equal(facts.get('pickers'), [row['args'][0] for row in picks]):
            raise ValueError('Picker inventory differs from actual port method calls')
        associations = [row for row in calls if row.get('method') == 'associate']
        if not equal(facts.get('associations'), [row['args'][0] for row in associations]):
            raise ValueError('Association inventory differs from actual port method calls')
        if actual is not None:
            for method in ('selectWindowPreview', 'associate'):
                entries = [row for row in calls if row.get('method') == method]
                transports = [row for row in facts.get('transportCalls', []) if row.get('method') == method]
                if len(transports) != len(entries) or len({row.get('responseId') for row in transports}) != len(transports):
                    raise ValueError('Picker/association calls lack one-to-one actual RPC responses')
                for call, transport in zip(entries, transports):
                    record = actual.get(transport.get('responseId'))
                    request = record.get('request') if record else None
                    if (not isinstance(request, dict)
                            or request.get('scenario') != DIAGNOSTIC_SCENES[COMPONENT_CASES[6]]
                            or request.get('action') != 'invoke'
                            or request.get('method') != method
                            or not equal(request.get('args'), call.get('args'))
                            or not equal(transport.get('args'), call.get('args'))
                            or not any(reply.get('id') == transport.get('responseId') for reply in facts['rpcMessages'])):
                        raise ValueError('Picker/association invocation borrowed a different RPC method, scenario or request')
    expected = {'pickerCallsBeforeIntent': len(before['pickers']),
                'pickerCallsAfterIntent': len(after['pickers']),
                'associationCallsBeforeIntent': sum(row.get('method') == 'associate'
                                                   for row in before['methodCalls'])}
    if any(type(observations.get(key)) is not int or observations[key] != value
           for key, value in expected.items()):
        raise ValueError('Picker finite counts differ from actual before/after facts')


def inspect_scenes(root, report, directory, nonce, source, producer):
    scenes = report.get('scenes')
    registry = [{'id': identity, 'journey_id': journey, 'name': name}
                for identity, (journey, name) in COMPONENT_SCENES.items()]
    if (report.get('component_scene_registry') != registry or not isinstance(scenes, list)
            or [row.get('id') for row in scenes] != list(COMPONENT_SCENES)):
        raise ValueError('Exact complete thirty-eight scene registry/order required')
    rpc = inspect_rpc(root, report, directory, nonce, source, producer)
    judgments, scene_refs = [], []
    for row in scenes:
        identity = row['id']
        journey, name = COMPONENT_SCENES[identity]
        trace = load(within(root, row['trace'], directory))
        if (trace.get('kind') != 'binding-journey-scene-observations/v1' or trace.get('schema_version') != 1
                or trace.get('nonce') != nonce or trace.get('scene_id') != identity
                or trace.get('journey_id') != journey or trace.get('observations') != row.get('observations')
                or row.get('journey_id') != journey or row.get('name') != name):
            raise ValueError('Original scene observation identity differs')
        judgment = scene_verdict(identity, trace['observations'])
        capabilities = rpc['ready']['capabilities'] if rpc.get('ready') else {'scene_ids': [], 'methods': [], 'controls': []}
        missing_methods = [method for method in required_scene_methods(name) if method not in capabilities['methods']]
        capable = (identity in capabilities['scene_ids'] and {'reset', 'deliver'} <= set(capabilities['controls']) and not missing_methods)
        mode = trace['observations']['transport_mode']
        expected_mode = ('production_rpc' if capable else 'controlled_diagnostic'
                         if producer['manifest'].get('runtime_transport') is None and name in SCENE_FALLBACKS else 'unavailable')
        if mode != expected_mode:
            raise ValueError('Scene skipped available actual inputs or forged production capability: ' + identity)
        expected_blocked = ([] if capable else ['actual-production-rpc-input:' + identity]
                            + ['actual-production-rpc-method:' + method for method in missing_methods])
        if trace['observations']['steps']:
            inputs = actual_scene_inputs(trace['observations']['steps'])
            if trace['observations'].get('actual_inputs') != inputs:
                raise ValueError('Protected session/operation preconditions were fabricated instead of actual Rust input')
            if capable:
                for condition, prefix in [
                    (name in {'reuse', 'switch-cancel', 'cancel-candidate', 'revoke'} and not inputs['protected_session_id'], 'actual-protected-session-input:'),
                    (name == 'stale-events' and not inputs['current_operation_id'], 'actual-current-operation-input:'),
                    (name in {'close-operation', 'stale-events', 'real-stage-order'} and not inputs['committed_operation_ids'], 'actual-committed-operation-input:'),
                    (name in {'stale-review', 'stale-effects'} and not inputs['replacement_session_id'], 'actual-replacement-session-input:')]:
                    if condition:
                        expected_blocked.append(prefix + identity)
        if name == 'details' and trace['observations']['steps']:
            expected_blocked += inspect_source_details(trace['observations']['steps'][-1], rpc)
        if row.get('blocked_obligations') != expected_blocked or trace['observations'].get('blocked_obligations') != expected_blocked:
            raise ValueError('Scene missing/blocked obligations differ from actual producer capability')
        judgment['blocked_obligations'] = expected_blocked
        if judgment['status'] != row.get('status'):
            raise ValueError('Scene passed/blocked/failure differs from independent raw oracle: ' + identity)
        # Assertions are a report projection, never an author-selectable subset.
        if row.get('assertions') != judgment['assertions']:
            raise ValueError('Scene assertions were removed/rewritten: ' + identity)
        steps = trace['observations']['steps']
        if not steps:
            if row.get('screenshot') is not None or row.get('dom') is not None or trace.get('events') != []:
                raise ValueError('Unavailable scene fabricated screenshot or event execution')
        else:
            for step in steps:
                image = within(root, step['screenshot'], directory)
                if png_dimensions(image.read_bytes()) != (step['dom']['geometry']['width'], step['dom']['geometry']['height']):
                    raise ValueError('Original scene step PNG/viewport differs')
                html = within(root, step['html'], directory)
                if '<html' not in html.read_text().lower():
                    raise ValueError('Original complete scene DOM missing')
            if trace['events'] != steps[-1]['facts']['events']:
                raise ValueError('Scene final events differ from actual final checkpoint')
            png_dimensions(within(root, row['screenshot'], directory).read_bytes())
            within(root, row['dom'], directory)
        if trace['observations']['transport_mode'] == 'production_rpc':
            if rpc.get('ready') is None or identity not in rpc['ready']['capabilities']['scene_ids']:
                raise ValueError('Scene fabricated actual production RPC capability')
            actual_replies = {message['response']['id']: message for message in rpc['messages'] if message['request']['scenario'] == identity}
            for step in steps:
                replies = step['facts']['rpcMessages']
                if (not isinstance(replies, list) or not replies or replies[0].get('action') != 'reset'
                        or len({row['id'] for row in replies}) != len(replies)
                        or any(row['id'] not in actual_replies or actual_replies[row['id']]['response'] != row for row in replies)
                        or any(row.get('action') == 'reset' for row in replies[1:])):
                    raise ValueError('Scene rewrote actual Rust RPC responses/publication IDs')
                raw_effects = [event for reply in replies for event in reply['events']]
                observed_effects = [event['event'] for event in step['facts']['events'] if event.get('kind') == 'production_side_effect']
                if raw_effects != observed_effects:
                    raise ValueError('Scene manufactured backend commit/window-effect facts')
            if name in {'library-global-owner', 'scoped-reconnect-owner'}:
                judgment['library_database'] = inspect_library_owner_database(root, identity, steps, rpc, directory, nonce, source)
            inspect_post_checkpoint_facts(steps[-1]['facts'], trace.get('post_checkpoint_facts'), actual_replies)
        elif steps:
            inspect_post_checkpoint_facts(steps[-1]['facts'], trace.get('post_checkpoint_facts'))
        judgments.append(judgment)
        scene_refs.append({'id': identity, 'trace': row['trace']})
    layer_results = []
    for journey in dict.fromkeys(value[0] for value in COMPONENT_SCENES.values()):
        rows = [row for row in judgments if COMPONENT_SCENES[row['id']][0] == journey]
        status = 'failed' if any(row['status'] == 'failed' for row in rows) else 'blocked' if any(row['status'] == 'blocked' for row in rows) else 'passed'
        layer_results.append({'case_id': journey, 'layer': 'component', 'status': status,
                              'scene_ids': [row['id'] for row in rows]})
    return {'judgments': judgments, 'scene_refs': scene_refs, 'layer_results': layer_results, 'rpc': rpc}


def inspect_pending_capture(root, row, trace, directory, nonce, actual_rpc=None):
    """Bind finite pending observations, original DOM and actual raster cut.

    A bounded retry is for observation consistency, never for an expected UI
    outcome. Earlier inconsistent cuts remain original artifacts.
    """
    observations = trace['observations']
    refs = observations.get('captureRecords')
    if not isinstance(refs, list) or not 1 <= len(refs) <= 3:
        raise ValueError('Actual pending browser capture records are missing')
    records, cursor, previous_facts = [], 0, None
    actual = ({item['response']['id']: item for item in actual_rpc.get('messages', [])}
              if actual_rpc is not None else None)
    equal = lambda a, b: json.dumps(a, sort_keys=True) == json.dumps(b, sort_keys=True)

    def fact_chain(before, after):
        inspect_post_checkpoint_facts(before, after, actual)
        for key in ('associations', 'commands', 'windows', 'previewStarts', 'pickers'):
            if key == 'associations' or key in before or key in after:
                old, new = before.get(key), after.get(key)
                if not isinstance(old, list) or not isinstance(new, list) or not equal(new[:len(old)], old):
                    raise ValueError('Pending capture cumulative facts regressed: ' + key)
        for facts in (before, after):
            calls = [item for item in facts['events'] if item.get('kind') == 'port_call']
            associations = [item['args'][0] for item in calls if item.get('method') == 'associate']
            if not equal(calls, facts['methodCalls']) or not equal(associations, facts['associations']):
                raise ValueError('Pending associations differ from original actual port entries')
    for ordinal, reference in enumerate(refs, 1):
        record = load(within(root, reference, directory))
        if (record.get('kind') != 'binding-finite-pending-capture/v1'
                or type(record.get('schema_version')) is not int or record['schema_version'] != 1
                or record.get('nonce') != nonce or record.get('case_id') != row['id']
                or type(record.get('attempt')) is not int or record['attempt'] != ordinal):
            raise ValueError('Pending capture identity/attempt differs from actual case')
        before, after = record['before'], record['after']
        if (any(type(item.get('at_ms')) is not int for item in (before, after))
                or not cursor <= before['at_ms'] <= after['at_ms'] or before['at_ms'] <= 0):
            raise ValueError('Pending capture browser interval changed or overlapped')
        cursor = after['at_ms']
        for item in (before, after):
            ui = item['ui']
            if (set(ui) != {'visibleText', 'stageLabels', 'dialogVisible', 'duplicateSubmitAvailable'}
                    or not isinstance(ui['visibleText'], str)
                    or not isinstance(ui['stageLabels'], list)
                    or any(not isinstance(label, str) for label in ui['stageLabels'])
                    or any(type(ui[key]) is not bool for key in ('dialogVisible', 'duplicateSubmitAvailable'))
                    or not isinstance(item['facts'].get('events'), list)):
                raise ValueError('Pending browser facts are not typed original observations')
        fact_chain(before['facts'], after['facts'])
        if previous_facts is not None:
            fact_chain(previous_facts, before['facts'])
        previous_facts = after['facts']
        within(root, record['screenshot'], directory)
        if ordinal < len(refs) and before['ui'] == after['ui']:
            raise ValueError('Pending collector skipped an earlier consistent actual outcome')
        records.append(record)
    last = records[-1]
    ui, facts = last['before']['ui'], last['before']['facts']
    if (last['after']['ui'] != ui
            or any(observations.get(key) != ui[key] for key in ('visibleText', 'stageLabels', 'duplicateSubmitAvailable'))
            or facts['events'] != trace['events']
            or observations.get('associationCalls') != facts.get('associations')
            or not isinstance(facts.get('associations'), list)
            or observations.get('pending') is not (
                ui['dialogVisible'] and len(ui['stageLabels']) == 3
                and len(facts['associations']) == 1 and not ui['duplicateSubmitAvailable'])
            or observations.get('postCaptureFacts') != last['after']['facts']
            or last['after']['facts'].get('associations') != facts.get('associations')
            or last['screenshot']['sha256'] != row['screenshot']['sha256']):
        raise ValueError('Pending DOM/text/facts/raster were taken across different browser states')


def inspect_component(root, reference, nonce, expected_sources):
    report_path = checked_ref(root, reference, ignored=True)
    directory = report_path.parent
    report = load(report_path)
    if (type(report.get('schema_version')) is not int or report['schema_version'] != 1
            or report.get('kind') != 'binding-journey-component-execution/v1'
            or report.get('nonce') != nonce
            or report.get('source_before') != products(expected_sources)
            or report.get('source_after') != products(expected_sources)
            or report.get('source_stable') is not True
            or report.get('component_mount_succeeded') is not True
            or report.get('native_executed') is not False
            or report.get('product_acceptance_promoted') is not False
            or report.get('next_stage_allowed') is not False):
        raise ValueError('Real component execution/source/limited scope identity differs')
    if not isinstance(report.get('infrastructure_errors'), list) or not isinstance(report.get('browser_errors'), list):
        raise ValueError('Explicit infrastructure/browser error lists required')
    if report['infrastructure_errors'] or report['browser_errors']:
        raise ValueError('Component did not execute without infrastructure/browser errors')
    interval(report['started_at'], report['finished_at'])
    security = report.get('production_security_policy', {})
    policy_path = 'apps/desktop/src-tauri/tauri.conf.json'
    raw_policy = path_at(root, policy_path).read_bytes()
    policy_hash = sha(raw_policy)
    csp = json.loads(raw_policy)['app']['security']['csp']
    if (policy_hash != expected_sources.get(policy_path)
            or security != {'source_file': policy_path, 'source_sha256': policy_hash, 'csp': csp,
                            'delivery': 'document_response_header', 'violations': []}):
        raise ValueError('Actual production CSP source/delivery differs')
    headers = load(within(root, report['response_headers'], directory))
    if (headers.get('source_file') != policy_path or headers.get('source_sha256') != policy_hash
            or headers.get('headers', {}).get('content-security-policy') != csp
            or not re.fullmatch(r'http://127\.0\.0\.1:[0-9]+', headers.get('origin', ''))):
        raise ValueError('Actual delivered component response CSP differs')
    console = load(within(root, report['console'], directory))
    page_errors = load(within(root, report['page_errors'], directory))
    if (not isinstance(console, list) or any(x.get('type') == 'error' for x in console)
            or page_errors != []):
        raise ValueError('Actual browser console/page errors prohibit semantic RED admission')
    for field, path in [('runner', RUNNER), ('plan', PLAN)]:
        if report.get(field) != {'path': path, 'sha256': expected_sources.get(path)}:
            raise ValueError('Component ' + field + ' identity differs')
    producer = inspect_producer(root, report['manifest'], nonce, expected_sources)
    actual_rpc = inspect_rpc(root, report, directory, nonce, expected_sources, producer)
    if report.get('projection_values') != producer['manifest']['projection_file']:
        raise ValueError('Component projection differs from producer stdout identity')
    cases = report.get('cases')
    if (not isinstance(cases, list) or [x.get('id') for x in cases] != list(COMPONENT_CASES)
            or report.get('coverage', {}).get('executed_case_ids') != list(COMPONENT_CASES)
            or report['coverage'].get('no_global_BJ01_18_completion_claim') is not True):
        raise ValueError('Exact finite component case set/order required; missing/duplicate/skip forbidden')
    judgments = []
    case_refs = []
    for row in cases:
        trace_path = within(root, row['trace'], directory)
        trace = load(trace_path)
        if (trace.get('kind') != 'binding-journey-case-observations/v1'
                or type(trace.get('schema_version')) is not int or trace['schema_version'] != 1
                or trace.get('nonce') != nonce or trace.get('case_id') != row['id']
                or trace.get('observations') != row.get('observations')
                or not isinstance(trace.get('events'), list)):
            raise ValueError('Raw case observation identity differs')
        if producer['manifest'].get('runtime_transport') is not None:
            capabilities = actual_rpc['ready']['capabilities'] if actual_rpc.get('ready') else None
            number = COMPONENT_CASES.index(row['id'])
            missing = (['producer-ready'] if capabilities is None else
                       ([] if DIAGNOSTIC_SCENES[row['id']] in capabilities['scene_ids'] else ['scene:' + DIAGNOSTIC_SCENES[row['id']]])
                       + ['method:' + method for method in dict.fromkeys((*DIAGNOSTIC_METHODS[number], 'controlWindowPreview')) if method not in capabilities['methods']]
                       + ([] if 'reset' in capabilities['controls'] else ['control:reset']))
            if missing:
                if (row.get('status') != 'blocked' or row.get('assertions') != []
                        or row.get('screenshot') is not None or row.get('dom') is not None
                        or trace['observations'] != {'transport_mode': 'unavailable', 'blocked_obligations': missing}
                        or trace['events'] != []):
                    raise ValueError('Diagnostic skipped/forged actual Rust input capability')
                judgments.append({'id': row['id'], 'status': 'blocked', 'assertions': [],
                                  'failed_assertions': [], 'blocked_obligations': missing})
                continue
        judgment = observation_verdict(row['id'], trace['observations'])
        if row['id'] == COMPONENT_CASES[2]:
            inspect_pending_capture(root, row, trace, directory, nonce,
                                    actual_rpc if producer['manifest'].get('runtime_transport') is not None else None)
        if row['id'] == COMPONENT_CASES[6]:
            inspect_finite_picker(trace['observations'], trace['events'],
                                  actual_rpc if producer['manifest'].get('runtime_transport') is not None else None)
        if row['id'] == COMPONENT_CASES[4] and trace['observations'].get('productionDetailFixtureAvailable'):
            obligations = (producer['runtime_values'] or {}).get('source_detail_obligations')
            observed = trace['observations'].get('sourceDetails')
            if not obligations or not isinstance(observed, list) or len(observed) != len(obligations):
                raise ValueError('Typed source detail observations have no production obligations')
            for obligation, actual in zip(obligations, observed):
                if actual.get('source_id') != obligation['source_id'] or actual.get('rowCount') != 1 or len(actual.get('fields', [])) != len(obligation['fields']):
                    raise ValueError('Source detail observations differ from actual source identity')
                for expected, field in zip(obligation['fields'], actual['fields']):
                    if (any(field.get(k) != v for k, v in expected.items()) or field.get('expected') != expected['value']
                            or field.get('matchCount') != 1):
                        raise ValueError('Source detail field was substituted or not source-specific')
        assertions = row.get('assertions')
        if (not isinstance(assertions, list) or not assertions
                or any(not isinstance(x, dict) or type(x.get('passed')) is not bool
                       or not isinstance(x.get('id'), str) or not x['id'] for x in assertions)
                or len({x['id'] for x in assertions}) != len(assertions)
                or any(x['passed'] != (json.dumps(x.get('expected'), sort_keys=True) ==
                                      json.dumps(x.get('actual'), sort_keys=True)) for x in assertions)
                or row.get('status') != judgment['status']
                or row['status'] != ('failed' if not all(x['passed'] for x in assertions) else 'blocked' if judgment['blocked_obligations'] else 'passed')):
            raise ValueError('Component status/typed assertions contradict independently evaluated raw observations')
        image = within(root, row['screenshot'], directory)
        png_dimensions(image.read_bytes())
        dom = within(root, row['dom'], directory)
        if '<html' not in dom.read_text().lower():
            raise ValueError('Original retained HTML required')
        case_refs.append({'id': row['id'], 'trace': row['trace'], 'dom': row['dom'], 'screenshot': row['screenshot']})
        judgments.append(judgment)
    for reference in report.get('screenshots', []):
        png_dimensions(within(root, reference, directory).read_bytes())
    bundle_path = directory / 'bundle'
    actual_bundle = list(bundle_path.rglob('*'))
    if not bundle_path.is_dir() or any(path.is_symlink() for path in actual_bundle):
        raise ValueError('Actual compiled component bundle missing or contains symlink assets')
    bundle_refs = report.get('bundle_assets')
    if (not isinstance(bundle_refs, list) or not bundle_refs
            or len({row.get('path') for row in bundle_refs}) != len(bundle_refs)
            or {row.get('path') for row in bundle_refs} != {path.relative_to(root).as_posix() for path in actual_bundle if path.is_file()}):
        raise ValueError('Original complete component bundle asset inventory differs')
    for reference in bundle_refs:
        within(root, reference, directory)
    decoded = report.get('decoded_images')
    if not isinstance(decoded, list):
        raise ValueError('Explicit actual decoded-image evidence inventory required')
    for row in decoded:
        image = within(root, {'path': row['path'], 'sha256': row['sha256']}, directory)
        if (row.get('encoding') != 'png' or row.get('conversion') != 'actual-decoded-pixels-1:1-no-resize'
                or type(row.get('original_source')) is not str
                or png_dimensions(image.read_bytes()) != (row['natural_width'], row['natural_height'])):
            raise ValueError('Actual unscaled decoded preview image evidence differs')
    first = cases[0]['observations']
    if cases[0]['status'] != 'blocked':
        if first.get('transport_mode') == 'production_rpc':
            raw = next(row for row in cases if row['id'] == COMPONENT_CASES[0])
            events = load(checked_ref(root, raw['trace'], ignored=True))['events']
            projection = first.get('projection')
            responses = [item['response']['response'] for item in actual_rpc['messages']
                         if item['request']['scenario'] == DIAGNOSTIC_SCENES[COMPONENT_CASES[0]]
                         and item['request']['method'] in {'startWindowPreview', 'previewSession', 'selectWindowPreview'}]
            if (projection not in responses or first.get('productionPolicyInput') != producer['values']['selection_required']
                    or any(projection.get(key) != producer['values']['selection_required'].get(key) for key in ('state', 'errorCode', 'permission'))):
                raise ValueError('RPC first-confirmation input lost actual production response/policy provenance')
        elif first.get('projection') != producer['values']['selection_required']:
            raise ValueError('First selection-required component input was not the production Rust DTO')
    complete_scenes = inspect_scenes(root, report, directory, nonce, expected_sources, producer)
    discharge = scoped_source_discharge(next(row for row in cases if row['id'] == COMPONENT_CASES[4]),
                                       next(row for row in report['scenes'] if row['id'] == 'BJS-10-details'), nonce)
    if report.get('source_detail_discharge') != discharge:
        raise ValueError('Source detail discharge is not bound to this same-run independently checked typed source scene')
    if discharge:
        for judgment in judgments:
            if judgment['id'] == COMPONENT_CASES[4] and judgment['status'] == 'blocked':
                judgment.update(status='passed', retained_blocked_obligations=judgment['blocked_obligations'],
                                blocked_obligations=[], discharged_by=discharge)
    failed_scenes = any(row['status'] == 'failed' for row in complete_scenes['judgments'])
    blocked_scenes = any(row['status'] == 'blocked' for row in complete_scenes['judgments'] + judgments)
    expected_status = ('failed' if any(x['status'] == 'failed' for x in judgments) or failed_scenes
                       else 'blocked' if blocked_scenes else 'passed')
    if report.get('status') != expected_status:
        raise ValueError('Forged component completion status')
    identity = report.get('browser_identity')
    if (not isinstance(identity, dict) or identity.get('playwright') != '1.56.1'
            or not isinstance(identity.get('executable'), str)):
        raise ValueError('Locked actual Playwright/browser identity required')
    binary = Path(identity['executable'])
    if not binary.is_file() or binary.is_symlink() or sha(binary.read_bytes()) != identity.get('binarySha256'):
        raise ValueError('Actual browser executable bytes differ')
    executions = report.get('executions')
    if not isinstance(executions, list) or len(executions) != 1:
        raise ValueError('Actual real-component build receipt required')
    build = inspect_command(root, executions[0], directory)
    command = build['command']
    if (len(command) != 5 or Path(command[1]).resolve() != Path(root).resolve() / 'apps/desktop/node_modules/vite/bin/vite.js'
            or command[2:4] != ['build', '--config']
            or Path(command[4]).resolve() != directory / 'vite.config.mjs'):
        raise ValueError('Real component build command differs')
    return {'report': report, 'producer': producer, 'judgments': judgments, 'case_refs': case_refs,
            'scenes': complete_scenes, 'actual_rpc': actual_rpc}


PREVIEW_RAW_BRANCHES = {
    'bj_07_domain': ('resource_failure', 'target_lease_failure', 'authority_races'),
    FIXED_NATIVE_UNIT_GROUPS['EX-LIFE-NATIVE-UNIT'][0]: ('resource_failure', 'target_lease_failure'),
    FIXED_NATIVE_UNIT_GROUPS['EX-LIFE-NATIVE-UNIT'][1]: ('authority_races',),
    FIXED_NATIVE_UNIT_GROUPS['EX-LIFE-NATIVE-UNIT'][2]: ('singleflight',),
    FIXED_NATIVE_UNIT_GROUPS['EX-LIFE-NATIVE-UNIT'][3]: ('no_verified_window', 'first_confirmation', 'typed_busy'),
    FIXED_NATIVE_UNIT_GROUPS['EX-LIFE-NATIVE-UNIT'][4]: ('healthy', 'paused'),
    FIXED_NATIVE_UNIT_GROUPS['EX-LEASE-NATIVE-UNIT'][0]: ('retirement_admission',),
    FIXED_NATIVE_UNIT_GROUPS['EX-LEASE-NATIVE-UNIT'][1]: ('old_ack',),
}
ACTIVE_WITNESS_FIELDS = ('session_id', 'game_id', 'environment_key', 'target_exe_path', 'windows_pid',
                         'process_created_u64', 'architecture', 'broker_instance_id', 'resource', 'target_identity_sha256')
RESOURCE_WITNESS_FIELDS = ('snapshot_id', 'kind', 'version', 'manifest_sha256', 'contract_version', 'metadata_json')
AUTHORITY_MUTATION_FIELDS = frozenset({'profileId', 'serverInstanceId', 'profileGeneration',
    *[field for field in ACTIVE_WITNESS_FIELDS if field != 'resource'],
    *['resource.' + field for field in RESOURCE_WITNESS_FIELDS]})


def valid_active_witness(profile, witness):
    if (set(witness) != set(ACTIVE_WITNESS_FIELDS)
            or set(profile) != {'profileId','serverInstanceId','profileGeneration'}
            or type(profile['profileGeneration']) is not int or profile['profileGeneration'] < 0
            or any(type(profile.get(key)) is not str or not profile[key] for key in ('profileId','serverInstanceId'))
            or any(type(witness.get(key)) is not str or not witness[key] for key in
                   ('session_id','environment_key','target_exe_path','process_created_u64','architecture','broker_instance_id','target_identity_sha256'))
            or witness['game_id'] is not None and (type(witness['game_id']) is not str or not witness['game_id'])
            or type(witness['windows_pid']) is not int or witness['windows_pid'] <= 0
            or not re.fullmatch(r'0|[1-9][0-9]*', witness['process_created_u64'])
            or int(witness['process_created_u64']) >= 2**64
            or witness['architecture'] not in {'x86','x64'} or not re.fullmatch(r'[a-f0-9]{64}',witness['target_identity_sha256'])):
        return False
    resource = witness.get('resource')
    if (not isinstance(resource, dict) or set(resource) != set(RESOURCE_WITNESS_FIELDS)
            or any(type(resource[key]) is not str or not resource[key] for key in RESOURCE_WITNESS_FIELDS)
            or not re.fullmatch(r'[a-f0-9]{64}', resource['manifest_sha256'])):
        return False
    try: strict_json_text(resource['metadata_json'])
    except ValueError: return False
    return True


def valid_preview_lease(value):
    if not isinstance(value, dict): return False
    fields = {'lease_id','owner_kind','owner_id','generation','fence','epoch','filter','paused'}
    return (set(value) == fields and value['owner_kind'] in {'candidate','session'}
            and all(type(value[key]) is str and value[key] for key in ('lease_id','owner_id','generation','fence','filter'))
            and type(value['epoch']) is int and value['epoch'] >= 0 and type(value['paused']) is bool)


def inspect_preview_observations(raw, name, execution, nonce, source):
    """Recompute real Core/coordinator/lease observations, never producer verdicts."""
    prefix = 'LUNA_BINDING_PREVIEW_OBSERVATIONS_V1 '
    documents = []
    for line in raw.splitlines():
        harness = 'test ' + name + ' ... '
        if line.startswith(harness): line = line[len(harness):]
        if line.startswith(prefix): documents.append(strict_json_text(line[len(prefix):]))
    if not documents:
        return {'status': 'blocked', 'checks': [], 'errors': ['Actual loss-free Core/coordinator/lease producer observations not implemented']}
    if len(documents) != 1:
        raise ValueError('One exact current-test preview observation stream required')
    document = documents[0]
    if (set(document) != {'kind', 'schema_version', 'nonce', 'source_tree_sha256', 'test_name', 'pid', 'events'}
            or document['kind'] != 'binding-preview-observations/v1' or type(document['schema_version']) is not int or document['schema_version'] != 1
            or document['nonce'] != nonce or document['source_tree_sha256'] != tree_sha(source)
            or document['test_name'] != name or document['pid'] != execution['pid']):
        raise ValueError('Actual preview observations are not bound to this exact source/test/Popen PID/nonce')
    events = document['events']
    if not isinstance(events, list) or not events:
        raise ValueError('Loss-free ordered preview observations required')
    start, finish = interval(execution['started_at'], execution['finished_at'])
    previous = (0, 0, 0)
    for event in events:
        if (set(event) != {'sequence', 'monotonic_ns', 'wall_ms', 'thread_id', 'branch', 'kind', 'data'}
                or any(type(event.get(key)) is not int or event[key] <= 0 for key in ('sequence', 'monotonic_ns', 'wall_ms'))
                or event['sequence'] != previous[0] + 1 or event['monotonic_ns'] <= previous[1] or event['wall_ms'] < previous[2]
                or not int(start.timestamp()*1000) <= event['wall_ms'] <= int(finish.timestamp()*1000)
                or type(event['thread_id']) is not str or not event['thread_id']
                or not isinstance(event['data'], dict)):
            raise ValueError('Preview event identity/sequence/actual process interval is invalid')
        previous = event['sequence'], event['monotonic_ns'], event['wall_ms']
    if set(event['branch'] for event in events) != set(PREVIEW_RAW_BRANCHES[name]):
        raise ValueError('Code-owned complete preview branch inventory differs')
    checks = []
    def check(identity, passed):
        checks.append({'id': identity, 'passed': bool(passed)})
    for branch in PREVIEW_RAW_BRANCHES[name]:
        rows = [event for event in events if event['branch'] == branch]
        def one(kind):
            matches = [event for event in rows if event['kind'] == kind]
            if len(matches) != 1: raise ValueError('Actual preview branch lacks one event: ' + branch + '/' + kind)
            return matches[0]
        def data(kind): return one(kind)['data']
        if branch in {'resource_failure', 'target_lease_failure'}:
            before, after = data('s1_active_before'), data('s1_active_after')
            witness = before.get('active_witness') or {}
            profile = before.get('profile') or {}
            check(branch + '/full-active-witness', valid_active_witness(profile, witness))
            check(branch + '/s1-stays-actual-active', before == after)
            failure, fresh = one('s2_failed_before_stop'), data('fresh_authority_acquired')
            check(branch + '/prestop-causal-order', one('s1_active_before')['sequence'] < failure['sequence'] < one('s1_active_after')['sequence'])
            check(branch + '/no-stop-or-old-handoff', data('call_counters').get('stop_s1') == 0 and data('call_counters').get('old_handoff_callbacks') == 0)
            check(branch + '/fresh-full-authority', fresh.get('profile') == profile and fresh.get('active_witness') == witness)
            if name != 'bj_07_domain':
                check(branch + '/candidate-untouched', valid_preview_lease(data('candidate_before'))
                      and data('candidate_before') == data('candidate_after'))
                check(branch + '/subscribed-session-event-driver', data('call_counters').get('direct_ensure_calls') == 0
                      and data('call_counters').get('preview_session_polls') == 0 and data('call_counters').get('mounted_ui') == 0
                      and one('active_subscription_event')['sequence'] < one('session_collector_started')['sequence']
                      and data('session_collector_started').get('active_witness') == witness)
        elif branch == 'authority_races':
            attempts = [event['data'] for event in rows if event['kind'] == 'mutated_authority_attempt']
            def fields(authority):
                profile, witness = authority.get('profile') or {}, authority.get('active_witness') or {}
                flattened = {key: profile.get(key) for key in ('profileId','serverInstanceId','profileGeneration')}
                flattened.update({key: witness.get(key) for key in ACTIVE_WITNESS_FIELDS if key != 'resource'})
                flattened.update({'resource.' + key: (witness.get('resource') or {}).get(key) for key in RESOURCE_WITNESS_FIELDS})
                return flattened
            derived_mutations = []
            for attempt in attempts:
                current, attempted = attempt.get('current_authority') or {}, attempt.get('attempted_authority') or {}
                left, right = fields(current), fields(attempted)
                changed = {key for key in left if json.dumps(left[key], sort_keys=True) != json.dumps(right[key], sort_keys=True)}
                derived_mutations.append(next(iter(changed)) if len(changed) == 1
                                         and valid_active_witness(current.get('profile') or {}, current.get('active_witness') or {}) else None)
            check(branch + '/every-full-witness-fence', len(attempts) == len(AUTHORITY_MUTATION_FIELDS)
                  and set(derived_mutations) == AUTHORITY_MUTATION_FIELDS
                  and all(derived == row.get('mutated_field') for derived, row in zip(derived_mutations, attempts))
                  and all(row.get('effect_callbacks') == 0 and row.get('before') == row.get('after') for row in attempts))
            check(branch + '/fresh-authority-can-succeed', data('fresh_authority_attempt').get('effect_callbacks') == 1)
        elif branch in {'healthy','paused','no_verified_window','first_confirmation','typed_busy','singleflight'}:
            counts = data('call_counters')
            authority = data('active_authority')
            check(branch + '/actual-full-active-authority', valid_active_witness(authority.get('profile') or {}, authority.get('active_witness') or {}))
            check(branch + '/no-implicit-picker-or-frame', counts.get('picker_calls') == 0 and counts.get('synthetic_frames') == 0)
            check(branch + '/bounded-singleflight', type(counts.get('collector_start')) is int and 0 <= counts['collector_start'] <= 1
                  and type(counts.get('filter_start')) is int and 0 <= counts['filter_start'] <= 1)
            if branch in {'healthy','paused'}:
                check(branch + '/retain-existing-lease-and-state', valid_preview_lease(data('lease_before'))
                      and data('lease_before') == data('lease_after') and counts.get('collector_start') == 0
                      and data('lease_after').get('paused') is (branch == 'paused'))
            elif branch in {'no_verified_window','first_confirmation'}:
                check(branch + '/no-guessed-live-lease', counts.get('collector_start') == 0 and data('lease_after').get('session_lease') is None)
            else:
                check(branch + '/busy-release-then-admit', one('typed_busy')['sequence'] < one('release_ack')['sequence'] < one('session_admitted')['sequence'])
                admitted = data('session_admitted')
                check(branch + '/exact-current-session-admitted', valid_preview_lease(admitted)
                      and admitted['owner_kind'] == 'session' and admitted['owner_id'] == (authority.get('active_witness') or {}).get('session_id'))
        elif branch in {'retirement_admission','old_ack'}:
            snapshots = [event for event in rows if event['kind'] == 'lease_table_mutation']
            check(branch + '/unchanged-budget-at-every-mutation', bool(snapshots) and all(type(event['data'].get('active')) is int
                  and 0 <= event['data']['active'] <= 2 and type(event['data'].get('draining_pending')) is int
                  and 0 <= event['data']['draining_pending'] <= 1 for event in snapshots))
            if branch == 'retirement_admission':
                check(branch + '/guard-exits-before-blocking-stop', one('guard_exit')['sequence'] < one('collector_stop_dispatch')['sequence'] < one('release_ack')['sequence'] < one('session_admitted')['sequence'])
                check(branch + '/no-duplicate-admission', data('call_counters').get('collector_start') == 1 and data('call_counters').get('filter_start') == 1)
                check(branch + '/exact-retired-lease-ack', valid_preview_lease(data('retirement_mark'))
                      and data('retirement_mark') == data('release_ack') and valid_preview_lease(data('session_admitted')))
            else:
                check(branch + '/old-ack-cannot-mutate-new-owner', valid_preview_lease(data('lease_before'))
                      and data('lease_before') == data('lease_after') and data('call_counters').get('new_candidate_stops') == 0)
    return {'status': 'passed' if all(row['passed'] for row in checks) else 'failed', 'checks': checks, 'errors': []}


def inspect_fixed_native_units(root, reference, nonce, source):
    result = {identity: {'id': identity, 'status': 'blocked', 'checks': [], 'errors': [], 'artifacts': []}
              for identity in FIXED_NATIVE_UNIT_GROUPS}
    if reference is None:
        return list(result.values())
    path = checked_ref(root, reference, ignored=True)
    directory, report = path.parent, load(path)
    expected_groups = {identity: list(names) for identity, names in FIXED_NATIVE_UNIT_GROUPS.items()}
    if (report.get('kind') != 'binding-fixed-native-unit-executions/v1' or type(report.get('schema_version')) is not int or report.get('schema_version') != 1
            or report.get('nonce') != nonce or report.get('source_tree_sha256') != tree_sha(source)
            or report.get('source_map') != source or report.get('groups') != expected_groups
            or report.get('native_os_executed') is not False or report.get('product_acceptance_promoted') is not False):
        raise ValueError('Fixed Native unit execution/source registry differs')
    compilation_ref = report['compilation']
    compilation = load(within(root, compilation_ref, directory))
    code = compilation.get('exit_code')
    if type(code) is not int or compilation.get('command') != ['cargo', 'test', '--locked', '--offline', '-p', 'luna-reader', '--bin', 'luna-reader', '--no-run', '--message-format=json']:
        raise ValueError('Actual fixed Native locked compilation command required')
    inspect_command(root, compilation_ref, directory, expected_code=code)
    if code != 0 or compilation.get('timed_out') is not False or type(compilation.get('pid')) is not int or compilation['pid'] < 1:
        raise ValueError('Actual fixed Native unit compilation failed; infrastructure is not semantic RED')
    if report.get('binary') is None:
        if report.get('inventory') is not None or report.get('tests') != []:
            raise ValueError('Missing Native unit binary cannot fabricate inventory or tests')
        return list(result.values())
    binary = within(root, report['binary'], directory, limit=256 * 1024 * 1024)
    artifacts = []
    for line in checked_ref(root, compilation['stdout'], ignored=True).read_text().splitlines():
        try: item = strict_json_text(line)
        except ValueError: continue
        if item.get('reason') == 'compiler-artifact' and item.get('target', {}).get('name') == 'luna-reader' and item.get('profile', {}).get('test') is True and item.get('executable'):
            artifacts.append(item)
    if (len(artifacts) != 1 or artifacts[0] != report.get('cargo_artifact')
            or Path(artifacts[0]['manifest_path']).resolve() != root / 'apps/desktop/src-tauri/Cargo.toml'
            or Path(artifacts[0]['target']['src_path']).resolve() != root / 'apps/desktop/src-tauri/src/main.rs'):
        raise ValueError('Fixed Native unit binary is not the actual current locked Cargo target')
    listed = inspect_command(root, report['inventory'], directory)
    if listed['command'] != [str(binary), '--list', '--format', 'terse'] or listed.get('timed_out') is not False:
        raise ValueError('Actual full unfiltered Native unit inventory required')
    inventory = checked_ref(root, listed['stdout'], ignored=True).read_text()
    names = {line[:-6] for line in inventory.splitlines() if line.endswith(': test')}
    expected_names = [name for group in FIXED_NATIVE_UNIT_GROUPS.values() for name in group]
    if [row.get('test_name') for row in report.get('tests', [])] != expected_names:
        raise ValueError('All seven fixed Native unit names must remain registered')
    for row in report['tests']:
        name = row['test_name']
        group = next(identity for identity, names in FIXED_NATIVE_UNIT_GROUPS.items() if name in names)
        reference = row.get('execution')
        if name not in names:
            if row.get('status') != 'blocked' or reference is not None:
                raise ValueError('A missing exact Native unit cannot claim test execution')
            result[group]['checks'].append({'test_name': name, 'status': 'blocked'})
            continue
        actual = load(within(root, reference, directory))
        code = actual.get('exit_code')
        if (type(code) is not int or code not in (0, 101)
                or actual.get('command') != [str(binary), '--exact', name, '--nocapture']
                or type(actual.get('pid')) is not int or actual['pid'] < 1 or actual.get('timed_out') is not False):
            raise ValueError('Actual fixed Native exact test PID/argv/wait differs')
        inspect_command(root, reference, directory, expected_code=code)
        raw = checked_ref(root, actual['stdout'], ignored=True).read_text() + '\n' + checked_ref(root, actual['stderr'], ignored=True).read_text()
        passed = exact_test_outcome(raw, name, code)
        if actual.get('fixture_environment') != {'LUNA_BINDING_NONCE': nonce, 'LUNA_BINDING_SOURCE_TREE': tree_sha(source), 'LUNA_BINDING_OUTPUT': str(directory)}:
            raise ValueError('Fixed Native unit nonce/source/output environment differs')
        observed = inspect_preview_observations(raw, name, actual, nonce, source)
        result[group]['checks'].append({'test_name': name, 'status': observed['status'] if passed else 'failed',
                                        'observations': observed})
        result[group]['artifacts'].append(reference)
    for row in result.values():
        statuses = [check['status'] for check in row['checks']]
        row['status'] = 'failed' if 'failed' in statuses else 'blocked' if 'blocked' in statuses else 'passed'
        row['artifacts'] += [report['compilation'], report['inventory'], report['binary']]
    return list(result.values())


def inspect_rust_layers(root, reference, nonce, expected_sources, *, replay=False):
    path = checked_ref(root, reference, ignored=True)
    directory, report = path.parent, load(path)
    if (report.get('kind') != 'binding-journey-production-rust-layers/v1'
            or type(report.get('schema_version')) is not int or report['schema_version'] != 1
            or report.get('nonce') != nonce or report.get('source_map') != expected_sources
            or report.get('source_tree_sha256') != tree_sha(expected_sources)
            or report.get('product_acceptance_promoted') is not False
            or report.get('native_executed') is not False
            or [row.get('layer') for row in report.get('layers', [])] != list(RUST_LAYERS)):
        raise ValueError('Production Cargo layer execution/source identity differs')
    results, infra = [], []
    core_preview = {'status': 'blocked', 'checks': [], 'errors': ['Actual bj_07_domain preview lifecycle observations not implemented']}
    storage_v5 = {'status': 'blocked', 'errors': [],
                  'reason': 'registered production schema5 test producer not implemented',
                  'product_acceptance_promoted': False}
    for row in report['layers']:
        layer, cases = row['layer'], rust_cases(row['layer'])
        spec = RUST_LAYERS[layer]
        if row.get('source') != spec['source'] or row.get('cases') != cases:
            raise ValueError('Code-owned production Cargo target/case registry differs')
        if spec['source'] not in expected_sources:
            if row.get('status') != 'blocked' or row.get('executions') != [] or row.get('binary') is not None:
                raise ValueError('Missing production Rust target cannot claim execution')
            results += [{'case_id': case, 'layer': layer, 'status': 'blocked',
                         'reason': 'registered production test source not implemented'} for case in cases]
            continue
        refs = row.get('executions', [])
        if not refs:
            raise ValueError('Existing production Rust target must actually compile')
        compile_record = load(within(root, refs[0], directory))
        expected_command = ['cargo', 'test', '--locked', '--offline', '-p', spec['package'],
                            '--test', spec['target'], '--no-run', '--message-format=json']
        if compile_record.get('command') != expected_command:
            raise ValueError('Exact production test compilation command required')
        inspect_command(root, refs[0], directory, expected_code=compile_record['exit_code'])
        if compile_record['exit_code'] != 0:
            if row.get('status') != 'failed' or len(refs) != 1:
                raise ValueError('Failed compilation cannot manufacture test execution')
            infra.append(layer + ': production Cargo compilation failed')
            continue
        binary = within(root, row['binary'], directory, limit=256 * 1024 * 1024)
        raw_compile = checked_ref(root, compile_record['stdout'], ignored=True).read_text()
        artifacts = []
        for line in raw_compile.splitlines():
            try:
                message = json.loads(line)
            except ValueError:
                continue
            if (message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == spec['target']
                    and message.get('profile', {}).get('test') is True and message.get('executable')):
                artifacts.append(message)
        if (len(artifacts) != 1 or row.get('cargo_artifact') != artifacts[0]
                or artifacts[0].get('package_id') != cargo_package_id(root, spec['package'])
                or Path(artifacts[0]['target']['src_path']).resolve() != Path(root).resolve() / spec['source']
                or len(refs) != len(cases) + 2):
            raise ValueError('Actual compiled Cargo target/binary inventory differs')
        listing = inspect_command(root, refs[1], directory)
        if listing['command'] != [str(binary), '--list', '--format', 'terse']:
            raise ValueError('Actual compiled test inventory command required')
        inventory = checked_ref(root, listing['stdout'], ignored=True).read_text()
        names = re.findall(r'^([\w:]+): test$', inventory, re.M)
        if len(names) != len(set(names)) or not set(cases.values()) <= set(names):
            raise ValueError('Compiled production test inventory misses registered exact cases')
        case_executions = []
        for (case, name), execution_ref in zip(cases.items(), refs[2:]):
            execution = load(within(root, execution_ref, directory))
            code = execution.get('exit_code')
            if type(code) is not int or code not in (0, 101):
                raise ValueError('Actual Rust test exit code required')
            inspect_command(root, execution_ref, directory, expected_code=code)
            if execution['command'] != [str(binary), '--exact', name, '--nocapture']:
                raise ValueError('Exact unfiltered registered Rust case execution required')
            case_executions.append(execution)
            environment = None
            if layer == 'storage':
                environment = {'LUNA_BINDING_NONCE': nonce, 'LUNA_BINDING_SOURCE_TREE': tree_sha(expected_sources),
                               'LUNA_BINDING_OUTPUT': str(directory / 'storage-runtime')}
                if execution.get('fixture_environment') != environment:
                    raise ValueError('Actual production schema5 test nonce/source/output environment differs')
                if type(execution.get('pid')) is not int or execution['pid'] <= 0 or execution.get('timed_out') is not False:
                    raise ValueError('Actual schema5 parent producer Popen PID and successful wait required')
            raw = (checked_ref(root, execution['stdout'], ignored=True).read_text()
                   + '\n' + checked_ref(root, execution['stderr'], ignored=True).read_text())
            passed = exact_test_outcome(raw, name, code)
            if layer == 'domain' and case == 'BJ-07':
                if (execution.get('fixture_environment') != {'LUNA_BINDING_NONCE': nonce, 'LUNA_BINDING_SOURCE_TREE': tree_sha(expected_sources), 'LUNA_BINDING_OUTPUT': str(directory / 'domain-runtime')}
                        or type(execution.get('pid')) is not int or execution['pid'] < 1 or execution.get('timed_out') is not False):
                    raise ValueError('Actual Core lifecycle test Popen PID/nonce/source/output differs')
                core_preview = inspect_preview_observations(raw, name, execution, nonce, expected_sources)
                if not passed: core_preview['status'] = 'failed'
            if replay:
                replay_environment = None
                if environment is not None:
                    # A replay must never overwrite the original DB/proof files.
                    replay_output = __import__('tempfile').mkdtemp(prefix='schema5-replay-', dir=directory)
                    replay_environment = dict(__import__('os').environ, **environment)
                    replay_environment['LUNA_BINDING_OUTPUT'] = replay_output
                actual = __import__('subprocess').run(execution['command'], cwd=root, text=True,
                                                   env=replay_environment, capture_output=True, timeout=60)
                if actual.returncode != code or exact_test_outcome(actual.stdout + '\n' + actual.stderr, name, actual.returncode) != passed:
                    raise ValueError('Independent compiled production test replay differs')
            results.append({'case_id': case, 'layer': layer, 'status': 'passed' if passed else 'failed',
                            'test_name': name, 'execution': execution_ref, 'binary': row['binary']})
        if layer == 'storage':
            from storage_schema_v5 import inspect_runtime
            reference = row.get('storage_runtime')
            expected_file = directory / 'storage-runtime/storage-v5-runtime.json'
            if reference is None and expected_file.exists():
                raise ValueError('Actual emitted schema5 runtime evidence cannot be omitted')
            if reference is not None and within(root, reference, directory) != expected_file:
                raise ValueError('Only the registered schema5 producer output can be consumed')
            first_time = interval(case_executions[0]['started_at'], case_executions[0]['finished_at'])[0]
            last_time = interval(case_executions[-1]['started_at'], case_executions[-1]['finished_at'])[1]
            runtime_context = {'nonce': nonce, 'source_tree_sha256': tree_sha(expected_sources),
                               'artifact_sha256': row['binary']['sha256'],
                               'environment_sha256': sha(json.dumps(environment, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()),
                               'started_at_ms': int(first_time.timestamp() * 1000),
                               'finished_at_ms': int(last_time.timestamp() * 1000),
                               'test_names': list(cases.values()), 'executions': refs[2:],
                               'producer_source': spec['source'], 'binary': row['binary']}
            if reference is not None and load(expected_file).get('kind') == 'source-selection-schema5-runtime/v2':
                runtime_context.update(parent_executions=dict(zip(cases.values(), case_executions)),
                                       source_files=expected_sources)
            storage_v5 = inspect_runtime(root, reference, context=runtime_context)
            if storage_v5.get('status') not in {'passed', 'blocked', 'failed'}:
                raise ValueError('Schema5 independent runtime inspector returned an invalid status')
            if storage_v5['status'] == 'failed':
                raise ValueError('Actual schema5 runtime evidence rejected: ' + '; '.join(storage_v5.get('errors', [])))
            for result in results:
                if result['layer'] == 'storage' and result['status'] == 'passed':
                    result['status'] = storage_v5['status']
                    result['schema5_runtime'] = storage_v5
    return {'results': results, 'infrastructure_errors': infra, 'storage_v5_inspection': storage_v5,
            'core_preview_observations': core_preview}


def native_rows(path, table):
    # Table identifiers are selected only by this code-owned registry.
    allowed = {'sessions', 'capture_operations', 'hook_events', 'source_selection_sets',
               'source_selection_members', 'source_selections', 'reading_unit_revisions',
               'profile_meta', 'capture_sources'}
    if table not in allowed:
        raise ValueError('Unregistered Native database observation table')
    connection = sqlite3.connect(path.as_uri() + '?mode=ro&immutable=1', uri=True)
    connection.row_factory = sqlite3.Row
    try:
        if connection.execute('pragma integrity_check').fetchone()[0] != 'ok':
            raise ValueError('Native raw SQLite snapshot is corrupt')
        return [dict(row) for row in connection.execute('select * from ' + table)]
    finally:
        connection.close()


NATIVE_HUD_OUTCOMES = frozenset({('BJ-08', 'committed'), ('BJ-09', 'temporary'),
                                ('BJ-09', 'saved'), ('BJ-13', 'completed')})
NATIVE_PROGRESS_TRACES = {'waiting-observed': ('waiting', 'candidate'),
                          'fast-candidates': ('candidate',)}
NATIVE_REOPEN_EDGES = (('temporary', 'save_intent'), ('saved', 'saved_reopened'))


def native_role(case_id, step_id):
    """Only the source-owned registry can allow a missing Main window."""
    if case_id == 'BJ-06':
        valid = {'waiting', 'candidate'}
    else:
        valid = set(NATIVE_STEPS.get(case_id, ()))
    if step_id not in valid:
        raise ValueError('Unregistered Native case/step role')
    return 'hud' if (case_id, step_id) in NATIVE_HUD_OUTCOMES else 'main'


def native_inventory(case_id, steps):
    ids = tuple(step.get('id') for step in steps)
    if case_id == 'BJ-06':
        if ids not in NATIVE_PROGRESS_TRACES.values():
            raise ValueError('Exact waiting-observed or fast-candidates Native inventory required')
    elif ids != NATIVE_STEPS.get(case_id):
        raise ValueError('Exact registered Native case/step inventory required')
    for step in steps:
        if any(key in step for key in ('role', 'observation_role', 'allow_main_absent', 'milestones', 'branch')):
            raise ValueError('Native artifact cannot grant its own role/absence/branch exemption')
        native_role(case_id, step['id'])
    return ids


def native_windows(observed, step, case_id, pid, requested, known):
    role = native_role(case_id, step['id'])
    main_id, hud_id = step.get('main_window_id'), step.get('hud_window_id')
    if type(main_id) is not int or main_id <= 0 or main_id not in requested:
        raise ValueError('Native Main must be an exact independently requested window ID')
    windows = observed.get('windows', [])
    main = [w for w in windows if w.get('window_id') == main_id]
    if role == 'hud':
        if known.get('main') != (pid, main_id) or known.get('hud') != (pid, hud_id):
            raise ValueError('Hidden Main/HUD identity was not established by a prior actual Main observation')
        if len(main) > 1 or main and (main[0].get('pid') != pid or main[0].get('on_screen') is not False):
            raise ValueError('Successful Native outcome did not hide that exact Main')
        main = main or [{'pid': pid, 'window_id': main_id, 'on_screen': False}]
        hud = [w for w in windows if w.get('window_id') == hud_id]
        if (type(hud_id) is not int or hud_id == main_id or hud_id not in requested
                or len(hud) != 1 or hud[0].get('pid') != pid or hud[0].get('on_screen') is not True
                or not hud[0].get('capture', {}).get('png') or not hud[0]['capture'].get('ocr')):
            raise ValueError('Actual requested HUD original PNG/OCR is not OS-visible')
        return main[0], hud[0], hud_id
    if len(main) != 1 or main[0].get('pid') != pid or main[0].get('on_screen') is not True:
        raise ValueError('Native Main is absent, hidden or belongs to another process')
    if known.get('main') not in (None, (pid, main_id)):
        raise ValueError('Native Main window identity changed between observations')
    known['main'] = (pid, main_id)
    hud = [w for w in windows if w.get('window_id') == hud_id]
    if len(hud) == 1 and hud[0].get('pid') == pid and type(hud_id) is int and hud_id != main_id:
        if known.get('hud') not in (None, (pid, hud_id)):
            raise ValueError('Native HUD window identity changed')
        known['hud'] = (pid, hud_id)
    return main[0], hud[0] if len(hud) == 1 else None, main_id


def native_closed_events(events, bounds, observed_at_ms, watermark):
    actual = max((e['sequence'] for e in events if e['observed_at_ms'] <= observed_at_ms), default=0)
    if type(watermark) is not int or watermark != actual or not bounds['start_sequence'] <= watermark <= bounds['end_sequence']:
        raise ValueError('Native snapshot truncated actual in-interval journal events')
    if any(e['observed_at_ms'] > observed_at_ms for e in events if e['sequence'] <= watermark):
        raise ValueError('Native OS interval has no closed complete journal prefix')
    return [e for e in events if bounds['start_sequence'] <= e['sequence'] <= watermark]


def native_pairs(events, method):
    pending, pairs = {}, []
    for event in events:
        if event.get('method') != method:
            continue
        key = event.get('correlation_id')
        if event['action'] == 'port_call':
            if key in pending:
                raise ValueError('Duplicate in-flight actual Native method correlation')
            pending[key] = event
        elif event['action'] == 'port_return':
            call = pending.pop(key, None)
            if call is None or call['sequence'] >= event['sequence']:
                raise ValueError('Native method return has no prior actual call')
            pairs.append((call, event))
    return pairs


def native_profile(step):
    rows = step['tables'].get('profile_meta', [])
    if len(rows) != 1 or rows[0].get('runtime_state') != 'active':
        raise ValueError('Actual Native SQLite active profile authority missing')
    row = rows[0]
    if (not row.get('profile_id') or not row.get('server_instance_id')
            or type(row.get('runtime_generation')) is not int or not 1 <= row['runtime_generation'] <= 2**32-1):
        raise ValueError('Native full profile/server generation is malformed')
    return {'profileId': row['profile_id'], 'serverInstanceId': row['server_instance_id'],
            'profileGeneration': row['runtime_generation']}


def native_i64(value):
    if type(value) is int:
        integer = value
    elif isinstance(value, str) and re.fullmatch(r'0|[1-9][0-9]*', value):
        integer = int(value)
    else:
        raise ValueError('Native SQLite revision must be lossless canonical i64')
    if not 0 <= integer <= 2**63-1:
        raise ValueError('Native SQLite revision outside nonnegative i64')
    return integer


def native_current_head(step):
    sid = step['context'].get('session_id')
    rows = [r for r in step['tables']['source_selection_sets'] if r.get('session_id') == sid]
    if any(r.get('state') != 'sealed' for r in rows):
        raise ValueError('Native snapshot has an unstable unsealed selection')
    if not rows:
        return None
    versions = [native_i64(r['selection_set_revision']) for r in rows]
    if 0 in versions or len(set(versions)) != len(versions):
        raise ValueError('Native current selection head is duplicated or malformed')
    return rows[versions.index(max(versions))]


def native_sampling_closure(observed_at_ms, intervals, events, context, fence=None):
    """Actual command intervals, not a producer's free stable-snapshot flag."""
    if set(intervals) != {'os', 'database', 'lsof'}:
        raise ValueError('Native actual OS/database/FD sampling intervals missing')
    for pair in intervals.values():
        if len(pair) != 2 or any(type(n) is not int or n <= 0 for n in pair) or pair[0] > pair[1]:
            raise ValueError('Native actual sampling command interval malformed')
    if not intervals['os'][0] <= observed_at_ms <= intervals['os'][1]:
        raise ValueError('Native OS data is outside its actual observer command')
    start = min(p[0] for p in intervals.values())
    finish = max(p[1] for p in intervals.values())
    coverage = native_journal_read_fence(finish, events, context, fence)
    covered_until = coverage['finished_at_ms']
    readonly = {'snapshot', 'resources', 'targets', 'associationProgress', 'sourceSelection',
                'previewSession', 'gameWindowBinding', 'windowCapabilities', 'readGameCover'}
    identity = lambda e: tuple(e.get(k) for k in ('session_id','operation_id','selection_set_revision','active_target_identity_sha256'))
    for event in events:
        if not start <= event['observed_at_ms'] <= covered_until:
            continue
        if (event['action'] in {'persist_commit','show_hud_completed','hide_main_completed','window_effect_failed','operation_cancelled'}
                or event['action'] in {'port_call','port_return'} and event.get('method') not in readonly
                or event['action'] == 'context_transition' and identity(event) != identity(context)):
            raise ValueError('Native OS/DB/FD sampling interval crossed an actual authority mutation')
    # A mutating method already in flight can commit after an early DB copy.
    open_calls = {}
    for event in events:
        key = (event.get('correlation_id'), event.get('method'))
        if event['action'] == 'port_call' and event.get('method') not in readonly:
            open_calls[key] = event
        elif event['action'] == 'port_return':
            call = open_calls.pop(key, None)
            if call and call['observed_at_ms'] < start and event['observed_at_ms'] >= start:
                raise ValueError('Native snapshot crossed an in-flight mutating method')
    if any(call['observed_at_ms'] <= covered_until for call in open_calls.values()):
        raise ValueError('Native snapshot has no closed mutating-method return')
    return {'started_at_ms': start, 'finished_at_ms': finish, 'intervals': intervals,
            'journal_fence': coverage}


NATIVE_JOURNAL_FD_READER = "import fs from 'node:fs';\nimport path from 'node:path';\nimport {createHash} from 'node:crypto';\nconst started=Date.now(),file=process.argv[1],limit=64n*1024n*1024n;\nif(!path.isAbsolute(file)||path.resolve(file)!==file)throw Error('canonical journal path required');\nlet component=path.parse(file).root;\nfor(const part of file.slice(component.length).split(path.sep)){component=path.join(component,part);if(fs.lstatSync(component).isSymbolicLink())throw Error('journal path contains symlink');}\nconst fd=fs.openSync(file,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);\nconst facts=s=>({dev:String(s.dev),ino:String(s.ino),size:String(s.size),mtime_ns:String(s.mtimeNs),ctime_ns:String(s.ctimeNs)});\nlet before,after,bytes;\ntry{const a=fs.fstatSync(fd,{bigint:true});if(!a.isFile()||a.size<=0n||a.size>limit)throw Error('bounded regular journal required');before=facts(a);bytes=fs.readFileSync(fd);after=facts(fs.fstatSync(fd,{bigint:true}));if(bytes.length>Number(limit)||String(bytes.length)!==before.size||JSON.stringify(before)!==JSON.stringify(after))throw Error('journal changed while reading');}finally{fs.closeSync(fd);}\nconst executable=fs.realpathSync(process.execPath),executableSha=createHash('sha256').update(fs.readFileSync(executable)).digest('hex');\nconst result={kind:'native_app_journal_fd_archive/v1',schema_version:1,pid:process.pid,executable_path:executable,executable_sha256:executableSha,observed_started_at_ms:started,observed_finished_at_ms:Date.now(),path:file,before,after,bytes_base64:bytes.toString('base64'),sha256:createHash('sha256').update(bytes).digest('hex')};\nprocess.stdout.write(JSON.stringify(result)+'\\n');"


def native_lossless_decimal(value):
    if not isinstance(value, str) or not re.fullmatch(r'0|[1-9][0-9]*', value):
        raise ValueError('Native FD/stat integer must be a canonical decimal string')
    return int(value)


def native_archive_records(raw):
    """Parse the bytes actually read from the FD; no producer coverage flag."""
    if not isinstance(raw, bytes) or not raw.endswith(b'\n') or not 0 < len(raw) <= 64*1024*1024:
        raise ValueError('Native FD archive bytes are incomplete or unbounded')
    def unique(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError('Native FD archive contains duplicate JSON key')
            value[key] = item
        return value
    lines = raw.splitlines(keepends=True)
    rows = [json.loads(line, object_pairs_hook=unique) for line in lines]
    if len(rows) < 3 or rows[0].get('kind') != 'binding_journal_begin' or rows[-1].get('kind') != 'binding_journal_health':
        raise ValueError('Native actual FD archive lacks header/events/closed health')
    if sum(row.get('kind') == 'binding_journal_begin' for row in rows) != 1:
        raise ValueError('Native actual FD archive repeats its application header')
    event_rows = [(line, row) for line, row in zip(lines, rows) if row.get('kind') == 'binding_journal_event']
    if any(r.get('kind') not in {'binding_journal_begin','binding_journal_event','binding_journal_health'} for r in rows):
        raise ValueError('Native actual FD archive contains unknown records')
    count = len(event_rows)
    if not count or any(type(r.get('sequence')) is not int or r['sequence'] != n for n, (_, r) in enumerate(event_rows, 1)):
        raise ValueError('Native actual FD archive has a discontinuous event prefix')
    cohort_keys = ('nonce','pid','profile_id','boot_sec','boot_usec')
    cohort = {k: rows[0].get(k) for k in cohort_keys}
    if any(any(row.get(k) != value for k, value in cohort.items()) for row in rows[1:]):
        raise ValueError('Native actual FD archive changed app/nonce/profile/boot cohort')
    for index, row in enumerate(rows[1:], 1):
        if row.get('kind') == 'binding_journal_health':
            preceding = sum(r.get('kind') == 'binding_journal_event' for r in rows[:index])
            if (any(type(row.get(k)) is not int or row[k] != preceding for k in ('accepted','written','last_sequence'))
                    or any(type(row.get(k)) is not int or row[k] != 0 for k in ('dropped','invalid','write_failures'))):
                raise ValueError('Native actual FD archive has undrained/lost journal health')
    return {'header': rows[0], 'header_bytes': lines[0], 'events': [r for _, r in event_rows],
            'event_bytes': [line for line, _ in event_rows], 'health': rows[-1]}


def native_journal_read_fence(sample_finished, events, context, fence):
    """Require a real post-sample query and a stable same-app FD archive.

    The IO adapter below constructs this input from original command receipts.
    Pure verifier controls can exercise it, but cannot become Native evidence.
    """
    import base64
    keys = {'query_correlation_id','cohort','archive','profile','tables','held_before','held_after',
            'reader','fd_interval','archive_interval','archive_pid','executable','writer_interval'}
    if not isinstance(fence, dict) or set(fence) != keys:
        raise ValueError('Native sampling lacks the actual post-sample query/FD archive fence')
    cohort, archived, reader = fence['cohort'], fence['archive'], fence['reader']
    if (not isinstance(archived, dict) or set(archived) != {'sha256','size','health','records_base64'}
            or not isinstance(reader, dict) or set(reader) != {'kind','schema_version','pid','executable_path','executable_sha256',
                'observed_started_at_ms','observed_finished_at_ms','path','before','after','bytes_base64','sha256'}):
        raise ValueError('Native read fence contains free archive/reader coverage fields')
    if (set(cohort) != {'nonce','pid','profile_id','boot_sec','boot_usec'}
            or type(cohort['pid']) is not int or cohort['pid'] <= 0
            or not isinstance(cohort['nonce'], str) or not re.fullmatch(r'[0-9a-f]{32}', cohort['nonce'])):
        raise ValueError('Native read fence app/nonce cohort malformed')
    for key in ('fd_interval','archive_interval','writer_interval'):
        pair = fence[key]
        if not isinstance(pair, list) or len(pair) != 2 or any(type(n) is not int or n <= 0 for n in pair) or pair[0] > pair[1]:
            raise ValueError('Native actual read-fence command interval malformed')
    fd_start, fd_end = fence['fd_interval']
    read_start, read_end = fence['archive_interval']
    if not fence['writer_interval'][0] <= sample_finished < fd_start <= fd_end <= read_start <= read_end <= fence['writer_interval'][1]:
        raise ValueError('Native read-fence commands escaped the actual post-sample writer interval')
    if (reader.get('kind') != 'native_app_journal_fd_archive/v1' or reader.get('schema_version') != 1
            or type(fence['archive_pid']) is not int or fence['archive_pid'] <= 0
            or reader.get('pid') != fence['archive_pid']
            or reader.get('executable_path') != fence['executable'].get('path')
            or reader.get('executable_sha256') != fence['executable'].get('sha256')
            or not re.fullmatch(r'[0-9a-f]{64}', str(reader.get('executable_sha256')))
            or not Path(str(reader.get('executable_path'))).is_absolute()):
        raise ValueError('Native FD archive is not from the fixed actual reader process/executable')
    if (any(type(reader.get(k)) is not int for k in ('observed_started_at_ms','observed_finished_at_ms'))
            or not read_start <= reader['observed_started_at_ms'] <= reader['observed_finished_at_ms'] <= read_end):
        raise ValueError('Native FD reader observations escaped its real Popen interval')
    before, after = fence['held_before'], fence['held_after']
    if (set(before) != {'pid','fd','path','inode'} or set(after) != {'pid','fd','path','inode','dev'}
            or any(after.get(k) != v for k, v in before.items()) or before['pid'] != cohort['pid']
            or not isinstance(before['fd'], str) or not re.fullmatch(r'[0-9]+[a-z]*', before['fd'])
            or not Path(str(before['path'])).is_absolute() or reader.get('path') != before['path']):
        raise ValueError('Native post-query FD is not the same app-held original journal')
    stats = reader.get('before')
    if (not isinstance(stats, dict) or set(stats) != {'dev','ino','size','mtime_ns','ctime_ns'}
            or reader.get('after') != stats):
        raise ValueError('Native journal changed during the actual same-FD read')
    for value in stats.values():
        native_lossless_decimal(value)
    if (stats['ino'] != before['inode'] or stats['dev'] != after['dev']
            or native_lossless_decimal(stats['size']) > 64*1024*1024):
        raise ValueError('Native FD archive dev/inode differs from actual application lsof')
    try:
        raw = base64.b64decode(reader['bytes_base64'], validate=True)
        lines = archived['records_base64']
        if not isinstance(lines, list) or not lines or any(not isinstance(line, str) for line in lines):
            raise ValueError('Native original journal line inventory missing')
        original_lines = [base64.b64decode(line, validate=True) for line in lines]
        if any(not line.endswith(b'\n') or b'\n' in line[:-1] for line in original_lines):
            raise ValueError('Native original journal record must retain its exact single LF')
        full_raw = b''.join(original_lines)
    except (KeyError, ValueError, TypeError) as error:
        raise ValueError('Native actual FD/full-journal archive bytes missing') from error
    if (len(raw) != native_lossless_decimal(stats['size']) or sha(raw) != reader.get('sha256')
            or len(full_raw) != native_lossless_decimal(archived['size']) or sha(full_raw) != archived.get('sha256')):
        raise ValueError('Native FD/full archive original bytes SHA or size differs')
    prefix, full = native_archive_records(raw), native_archive_records(full_raw)
    if (prefix['header_bytes'] != full['header_bytes'] or prefix['header'] != full['header']
            or {k: full['header'].get(k) for k in cohort} != cohort
            or full['health'] != archived.get('health') or full['events'] != events
            or len(prefix['events']) > len(events)
            or prefix['event_bytes'] != full['event_bytes'][:len(prefix['events'])]):
        raise ValueError('Native FD archive is not the exact original full-journal event prefix')
    if (any(type(e.get('observed_at_ms')) is not int or e['observed_at_ms'] > reader['observed_finished_at_ms'] for e in prefix['events'])
            or any(e['observed_at_ms'] <= reader['observed_finished_at_ms'] for e in events[len(prefix['events']):])):
        raise ValueError('Native FD archive omitted an event within its actual read interval')
    correlation = fence['query_correlation_id']
    if not isinstance(correlation, str) or not correlation:
        raise ValueError('Native read fence actual opaque query correlation missing')
    queries = [e for e in prefix['events'] if e.get('correlation_id') == correlation and e.get('action') in {'port_call','port_return'}]
    method = 'sourceSelection' if context.get('session_id') is not None else 'snapshot'
    if (len(queries) != 2 or [e['action'] for e in queries] != ['port_call','port_return']
            or any(e.get('method') != method for e in queries)):
        raise ValueError('Native post-sampling readonly query actual call/return is absent or crossed')
    call, returned = queries
    if not sample_finished < call['observed_at_ms'] <= returned['observed_at_ms'] < fd_start:
        raise ValueError('Native readonly query does not fence the end of actual OS/DB/FD sampling')
    current_profile = native_profile({'tables': fence['tables']})
    if fence['profile'] != current_profile or cohort['profile_id'] != current_profile['profileId']:
        raise ValueError('Native post-sampling probe full profile differs from actual SQLite')
    if method == 'sourceSelection':
        if call.get('args') != {'sessionId': context['session_id'], 'expectedProfileGeneration': current_profile['profileGeneration']}:
            raise ValueError('Native post-sampling sourceSelection has stale session/profile arguments')
        projection_step = {'tables': fence['tables'], 'context': context, 'journal': queries}
        native_source_projection(projection_step)
        projection = returned.get('response', {})
        head = native_current_head(projection_step)
        if head is not None:
            members = sorted((m for m in fence['tables']['source_selection_members'] if m.get('session_id') == context['session_id']
                              and native_i64(m['selection_set_revision']) == native_i64(head['selection_set_revision'])), key=lambda m: m['member_ordinal'])
            actual = {'sessionId': context['session_id'], 'setRevision': str(native_i64(head['selection_set_revision'])),
                      'operationId': head['operation_id'], 'persistence': head['persistence_kind'],
                      'selectedAtMs': str(head['selected_at_ms']),
                      'members': [{'sourceId': m['source_id'], 'selectionRevision': m['selection_revision'], 'ordinal': m['member_ordinal']} for m in members]}
            if (projection.get('current') != actual or not 1 <= len(members) <= 64 or len(members) != head['member_count']
                    or [m['member_ordinal'] for m in members] != list(range(len(members)))
                    or len({m['source_id'] for m in members}) != len(members)
                    or any(type(m['selection_revision']) is not int or not 1 <= m['selection_revision'] <= 2**32-1 for m in members)):
                raise ValueError('Native post-sample query current set members/persistence differ from SQLite')
    else:
        value = returned.get('response', {})
        hooks = [row for row in fence['tables']['sessions'] if row.get('source_kind') == 'hook']
        latest = max(hooks, key=lambda row: (native_i64(row['started_at_ms']), row['session_id'])) if hooks else None
        session = value.get('session', 'missing')
        expected = None if latest is None else {'sessionId': latest['session_id'], 'gameId': latest['game_id'],
                                                'sourceKind': latest['source_kind'], 'state': latest['state']}
        revision = value.get('commitRevision')
        if (call.get('args') != {} or any(value.get(k) != v for k, v in current_profile.items())
                or (session is not None if expected is None else not isinstance(session, dict) or any(session.get(k) != v for k, v in expected.items()))
                or type(revision) is not int or not 0 <= revision <= 2**32-1
                or native_i64(revision) != native_i64(fence['tables']['profile_meta'][0].get('commit_revision'))):
            raise ValueError('Native no-session snapshot query/profile differs from actual sampled SQLite')
    return {'query_correlation_id': correlation, 'query_sequence': returned['sequence'],
            'finished_at_ms': reader['observed_finished_at_ms'], 'archive_sha256': reader['sha256'],
            'last_sequence': prefix['health']['last_sequence'], 'fd': before['fd']}


def native_lsof_journal(text, pid, journal_path, *, device=False):
    """Read actual lsof fields without converting FD/dev/inode through float."""
    if [line for line in text.splitlines() if line.startswith('p')] != ['p'+str(pid)]:
        raise ValueError('Native journal FD observation belongs to another application PID')
    entries, row = [], {}
    for line in text.splitlines():
        if not line:
            continue
        key, value = line[0], line[1:]
        if key == 'f':
            if row:
                entries.append(row)
            row = {'pid': pid, 'fd': value}
        elif key == 'n':
            row['path'] = value
        elif key == 'i':
            row['inode'] = value
        elif key == 'D':
            if not re.fullmatch(r'0x[0-9a-fA-F]+', value):
                raise ValueError('Native journal lsof device is not exact hexadecimal')
            row['dev'] = str(int(value, 16))
    if row:
        entries.append(row)
    found = [entry for entry in entries if entry.get('path') == journal_path]
    if len(found) != 1:
        raise ValueError('Native app-held journal FD is missing or ambiguous')
    entry = found[0]
    native_lossless_decimal(entry.get('inode'))
    if device:
        native_lossless_decimal(entry.get('dev'))
    return entry


def native_journal_read_command(journal_path):
    import shutil
    node = shutil.which('node')
    if node is None:
        raise ValueError('Fixed Native archive reader executable unavailable')
    return [str(Path(node).resolve(strict=True)), '--input-type=module', '--eval',
            NATIVE_JOURNAL_FD_READER, journal_path]


def inspect_native_journal_fence(root, raw, directory, journal, tables, context, opened_text, pid,
                                 started, finished):
    """Only original fixed commands may construct the private sampling proof."""
    proof = raw.get('journal_fence')
    if not isinstance(proof, dict) or set(proof) != {'query_correlation_id','fd_execution','archive_execution'}:
        raise ValueError('Native writer has not produced the actual post-sample query/FD archive evidence')
    fd = inspect_command(root, proof['fd_execution'], directory)
    if fd['command'] != ['/usr/sbin/lsof','-nP','-p',str(pid),'-F','pfDin']:
        raise ValueError('Native post-query FD command is not the fixed app lsof observation')
    after_text = within(root, fd['stdout'], directory).read_text()
    before = native_lsof_journal(opened_text, pid, raw['actual_journal_path'])
    after = native_lsof_journal(after_text, pid, raw['actual_journal_path'], device=True)
    reader_command = inspect_command(root, proof['archive_execution'], directory)
    expected_command = native_journal_read_command(raw['actual_journal_path'])
    executable = Path(expected_command[0])
    if reader_command['command'] != expected_command:
        raise ValueError('Native actual archive command differs from the fixed readonly FD reader')
    reader = load(within(root, reader_command['stdout'], directory, limit=128*1024*1024))
    def milliseconds(record):
        a, b = interval(record['started_at'], record['finished_at'])
        return [int(a.timestamp()*1000), int(b.timestamp()*1000)]
    header = journal[0]
    return {'query_correlation_id': proof['query_correlation_id'],
            'cohort': {key: header.get(key) for key in ('nonce','pid','profile_id','boot_sec','boot_usec')},
            'archive': header['__native_archive'], 'profile': native_profile({'tables': tables}),
            'tables': tables, 'held_before': before, 'held_after': after, 'reader': reader,
            'fd_interval': milliseconds(fd), 'archive_interval': milliseconds(reader_command),
            'archive_pid': reader_command.get('pid'), 'executable': {'path': str(executable), 'sha256': sha(executable.read_bytes())},
            'writer_interval': [started, finished]}


def native_same_database_owner(before, after):
    a, b = before.get('database_owner'), after.get('database_owner')
    if a != b:
        raise ValueError('Native observations borrowed another app-open database/FD cohort')


def native_selection_receipt(step):
    """Join actual method args, commit receipt, full profile and immutable DB rows."""
    context, events = step['context'], step['journal']
    profile = native_profile(step)
    op, sid, revision = (context.get(k) for k in ('operation_id', 'session_id', 'selection_set_revision'))
    if not op or not sid or not isinstance(revision, str) or not re.fullmatch(r'[1-9][0-9]*', revision):
        raise ValueError('Current Native durable selection authority missing')
    commits = [e for e in events if e['action'] == 'persist_commit' and e.get('operation_id') == op]
    if len(commits) != 1:
        raise ValueError('One actual current-operation durable commit required')
    commit = commits[0]
    receipt = commit.get('response', {})
    authority = {'profile': profile, 'sessionId': sid, 'setRevision': revision, 'operationId': op}
    if (receipt.get('profile') != profile or receipt.get('effectAuthority') != authority
            or receipt.get('operationId') != op or receipt.get('replayed') is not False):
        raise ValueError('Native commit receipt full profile/operation authority differs')
    selection = receipt.get('selection', {})
    rows = [r for r in step['tables']['source_selection_sets'] if r.get('operation_id') == op
            and r.get('session_id') == sid and str(r.get('selection_set_revision')) == revision]
    if len(rows) != 1 or rows[0].get('state') != 'sealed' or rows[0].get('origin_kind') != 'explicit_set':
        raise ValueError('Native current sealed selection is not present in actual SQLite')
    row = rows[0]
    if native_current_head(step) != row:
        raise ValueError('Native receipt/context is not the current sealed selection head')
    members = sorted((r for r in step['tables']['source_selection_members'] if r.get('session_id') == sid
                      and str(r.get('selection_set_revision')) == revision), key=lambda r: r['member_ordinal'])
    projected = [{'sourceId': r['source_id'], 'selectionRevision': r['selection_revision'], 'ordinal': r['member_ordinal']} for r in members]
    if (not 1 <= len(members) <= 64 or len(members) != row.get('member_count')
            or any(type(r.get('selection_revision')) is not int or not 1 <= r['selection_revision'] <= 2**32-1 for r in members)
            or [r['member_ordinal'] for r in members] != list(range(len(members)))
            or len({r['source_id'] for r in members}) != len(members)
            or selection != {'sessionId': sid, 'setRevision': revision, 'operationId': op,
                             'persistence': row['persistence_kind'], 'selectedAtMs': str(row['selected_at_ms']), 'members': projected}):
        raise ValueError('Native commit members/persistence differ from actual immutable SQLite set')
    sessions = [r for r in step['tables']['sessions'] if r.get('session_id') == sid]
    if len(sessions) != 1 or sessions[0].get('target_identity_sha256') != context.get('active_target_identity_sha256'):
        raise ValueError('Native selection active session/target differs')
    pairs = [(a, b) for a, b in native_pairs(events, 'beginReading') if a.get('args', {}).get('operationId') == op]
    if len(pairs) != 1:
        raise ValueError('Native durable commit needs one actual beginReading request/return')
    call, returned = pairs[0]
    args = call.get('args', {})
    if (args.get('sessionId') != sid or args.get('expectedProfileGeneration') != profile['profileGeneration']
            or args.get('sourceIds') != [m['source_id'] for m in members] or type(args.get('remember')) is not bool
            or args['remember'] != (row['persistence_kind'] == 'saved_snapshot')
            or args.get('expectedSetRevision') != (None if int(revision) == 1 else str(int(revision) - 1))
            or not call['sequence'] < commit['sequence'] < returned['sequence']
            or not call['observed_at_ms'] <= commit['observed_at_ms'] <= returned['observed_at_ms'] <= step['os']['observed_at_ms']
            or returned.get('response', {}).get('selection') != receipt):
        raise ValueError('Native actual beginReading intent/return differs from durable receipt')
    for member in members:
        sources = [r for r in step['tables'].get('capture_sources', []) if r.get('source_id') == member['source_id'] and r.get('session_id') == sid]
        selections = [r for r in step['tables'].get('source_selections', []) if r.get('source_id') == member['source_id']
                      and r.get('session_id') == sid and r.get('selection_revision') == member['selection_revision']]
        if len(sources) != 1 or len(selections) != 1 or selections[0].get('persistence_kind') != row['persistence_kind']:
            raise ValueError('Native set lost actual source/member lineage')
    return {'commit': commit, 'receipt': receipt, 'authority': authority, 'row': row,
            'members': members, 'call': call, 'returned': returned}


def native_success_outcome(before, outcome):
    proof = native_selection_receipt(outcome)
    native_same_database_owner(before, outcome)
    if (native_profile(before) != native_profile(outcome)
            or before['context'].get('session_id') != outcome['context'].get('session_id')
            or before['context'].get('active_target_identity_sha256') != outcome['context'].get('active_target_identity_sha256')):
        raise ValueError('Native before/outcome current profile/session/target authority changed')
    identity = lambda s: (s['process']['pid'], s['process']['start_sec'], s['process']['start_usec'],
                          s['process']['executable'], s['os']['boot_sec'], s['os']['boot_usec'])
    if (identity(before) != identity(outcome) or before.get('hud', {}).get('window_id') != outcome.get('hud', {}).get('window_id')
            or outcome.get('hud', {}).get('pid') != outcome['process']['pid']):
        raise ValueError('Native outcome process/boot/HUD identity differs from actual before observation')
    if before['main'].get('on_screen') is not True or before['main'].get('window_id') != outcome['main'].get('window_id'):
        raise ValueError('Native successful outcome lacks previously observed actual Main identity')
    if (outcome['main'].get('on_screen') is not False or outcome.get('hud', {}).get('on_screen') is not True
            or outcome['os']['observed_at_ms'] <= before['os']['observed_at_ms']):
        raise ValueError('Native outcome must show actual HUD and known Main hidden at its real later time')
    op, sid, revision = (outcome['context'][k] for k in ('operation_id', 'session_id', 'selection_set_revision'))
    relevant = [e for e in outcome['journal'] if e.get('correlation_id') == op]
    effect_events = [e for e in relevant if e['action'] in {'show_hud_completed', 'hide_main_completed', 'window_effect_failed'}]
    attempts = [(proof['call'], proof['returned'])] + native_pairs(relevant, 'retryReadingWindowEffects')
    successful = []
    for call, returned in attempts:
        if call.get('method') == 'retryReadingWindowEffects' and call.get('args') != proof['authority']:
            raise ValueError('Native retry authority differs from original durable selection')
        effects = returned.get('response', {})
        if call.get('method') == 'beginReading':
            effects = effects.get('effects', {})
        if effects.get('authority') != proof['authority']:
            raise ValueError('Native window result does not carry full original authority')
        current = [e for e in effect_events if call['sequence'] < e['sequence'] < returned['sequence']]
        times = [call['observed_at_ms']] + [e['observed_at_ms'] for e in current] + [returned['observed_at_ms']]
        if times != sorted(times) or returned['observed_at_ms'] > outcome['os']['observed_at_ms']:
            raise ValueError('Native guarded effect timestamps do not close their actual call/return interval')
        for event in current:
            if (event.get('operation_id'), event.get('session_id'), event.get('selection_set_revision')) != (op, sid, revision):
                raise ValueError('Native effect journal context is stale or belongs to another selection')
            if event.get('active_target_identity_sha256') != outcome['context']['active_target_identity_sha256']:
                raise ValueError('Native effect actual target identity changed')
        shows = [e for e in current if e['action'] == 'show_hud_completed' and e.get('effect') == 'show_hud' and e.get('error_code') is None]
        hides = [e for e in current if e['action'] == 'hide_main_completed' and e.get('effect') == 'hide_main' and e.get('error_code') is None]
        show_ok = effects.get('showHud') == {'state': 'succeeded', 'error': None}
        hide_ok = effects.get('hideMain') == {'state': 'succeeded', 'error': None}
        expected_events = []
        for key, effect, completed in [('showHud', 'show_hud', 'show_hud_completed'), ('hideMain', 'hide_main', 'hide_main_completed')]:
            result = effects.get(key, {})
            if result.get('state') == 'succeeded' and result.get('error') is None:
                expected_events.append((completed, effect, None))
            elif result.get('state') == 'failed' and isinstance(result.get('error'), dict) and result['error'].get('code'):
                expected_events.append(('window_effect_failed', effect, result['error']['code']))
            elif result != {'state': 'not_attempted', 'error': None}:
                raise ValueError('Native actual effect result state/error malformed')
        if [(e['action'], e.get('effect'), e.get('error_code')) for e in current] != expected_events:
            raise ValueError('Native effect success/failure callbacks differ from actual return')
        if len(shows) != int(show_ok) or len(hides) != int(hide_ok):
            raise ValueError('Native window-effect return differs from actual guarded effect journal')
        if hides and (not shows or not proof['commit']['sequence'] < shows[0]['sequence'] < hides[0]['sequence']):
            raise ValueError('Actual durable commit/show-success/hide-success order is absent')
        if show_ok and hide_ok and effects.get('retryable') is False:
            successful.append(returned)
        elif effects.get('retryable') is not True:
            raise ValueError('Incomplete Native effects must retain truthful retryable outcome')
    if not successful or any(not any(a['sequence'] < e['sequence'] < b['sequence'] for a, b in attempts) for e in effect_events):
        raise ValueError('Native outcome lacks authorized effects or used later manual show/hide to repair them')
    if successful[-1]['sequence'] != attempts[-1][1]['sequence']:
        raise ValueError('Later failed Native effect attempt cannot inherit an earlier success')
    watermark, observed = before['raw']['journal_end_sequence'], before['os']['observed_at_ms']
    fresh = lambda event: event['sequence'] > watermark and event['observed_at_ms'] > observed
    if fresh(proof['call']) and fresh(proof['commit']):
        if any(r.get('operation_id') == op for r in before['tables']['source_selection_sets']):
            raise ValueError('Fresh Native handoff reused an already committed operation')
        previous = native_current_head(before)
        expected_head = None if previous is None else str(native_i64(previous['selection_set_revision']))
        if proof['call']['args'].get('expectedSetRevision') != expected_head:
            raise ValueError('Native new commit CAS differs from actual before snapshot head')
    else:
        # A failed already-durable effect may be retried, but is not a new commit.
        prior = native_selection_receipt(before)
        original = prior['returned'].get('response', {}).get('effects', {})
        retries = [(a,b) for a,b in attempts if a.get('method') == 'retryReadingWindowEffects' and fresh(a)]
        if (prior['authority'] != proof['authority'] or prior['commit'] != proof['commit']
                or original.get('retryable') is not True or not retries
                or any(e['action'] == 'persist_commit' for e in outcome['journal'] if e['sequence'] > watermark)
                or any(b['sequence'] <= watermark and b.get('response', {}).get('retryable') is False
                       for a,b in attempts if a.get('method') == 'retryReadingWindowEffects')):
            raise ValueError('Native handoff reused historical success instead of a fresh commit or failed-effect retry')
        if retries[-1][1]['sequence'] != attempts[-1][1]['sequence']:
            raise ValueError('Native failed-effect retry is not the final actual attempt')
    return proof


def native_action_command(action, pid):
    if type(pid) is not int or pid <= 0:
        raise ValueError('Native normal action must target actual app PID')
    body = 'set frontmost to true\nkeystroke "1" using command down'
    if action == 'open_current_sources':
        body = ('set hits to {}\nrepeat with element in (entire contents of window "Luna")\n'
                'try\nif role of element is "AXButton" and name of element is "重新选择文本流…" then set end of hits to element\n'
                'end try\nend repeat\nif (count hits) is not 1 then error "source button identity ambiguous"\nclick item 1 of hits')
    elif action != 'normal_show_main':
        raise ValueError('Unregistered Native normal action')
    script = ('tell application "System Events"\ntell (first application process whose unix id is ' + str(pid)
              + ')\n' + body + '\nend tell\nend tell')
    return ['/usr/bin/osascript', '-e', script]


def native_saved_descriptor(step, row):
    value = json.loads(row['match_snapshot_json'])
    if (value.get('schemaVersion') != 1 or value.get('matchCapability') != 'requires_review'
            or value.get('targetIdentitySha256') != step['context']['active_target_identity_sha256']):
        raise ValueError('Saved snapshot schema/capability/actual target identity differs')
    members = sorted((m for m in step['tables']['source_selection_members']
                      if m['session_id'] == row['session_id'] and m['selection_set_revision'] == row['selection_set_revision']),
                     key=lambda m: m['member_ordinal'])
    actual = []
    for member in members:
        sources = [s for s in step['tables']['capture_sources'] if s['session_id'] == row['session_id'] and s['source_id'] == member['source_id']]
        if len(sources) != 1:
            raise ValueError('Saved descriptor has no unique actual source')
        source = sources[0]
        for key in ('hook_address_u64', 'context_u64', 'subcontext_u64'):
            val = source.get(key)
            if not isinstance(val, str) or not re.fullmatch(r'0|[1-9][0-9]*', val) or int(val) > 2**64-1:
                raise ValueError('Saved actual source u64 identity is rounded or noncanonical')
        actual.append({'sourceId': source['source_id'], 'streamIdentitySha256': source['stream_identity_sha256'],
                       'hookName': source['hook_name'], 'hookCode': source['hook_code'], 'encoding': source['encoding'],
                       'address': source['hook_address_u64'], 'context': source['context_u64'], 'context2': source['subcontext_u64'],
                       'identityExtensionsJson': source['identity_extensions_json']})
    if not actual or value.get('sources') != actual or len(actual) != row['member_count']:
        raise ValueError('Saved full descriptors/members do not match actual source lineage')
    return value


def native_source_projection(step):
    profile, sid = native_profile(step), step['context'].get('session_id')
    pairs = [(a, b) for a, b in native_pairs(step['journal'], 'sourceSelection')
             if a.get('args', {}).get('sessionId') == sid]
    if not pairs:
        raise ValueError('Actual current-session sourceSelection method evidence missing')
    call, returned = pairs[-1]
    projection = returned.get('response', {})
    if (call['args'].get('expectedProfileGeneration') != profile['profileGeneration']
            or projection.get('profile') != profile or projection.get('sessionId') != sid):
        raise ValueError('Native source projection full profile/session differs')
    head = native_current_head(step)
    current = projection.get('current')
    if ((head is None) != (current is None) or head is not None and (
            current.get('sessionId') != sid or current.get('setRevision') != str(native_i64(head['selection_set_revision']))
            or current.get('operationId') != head['operation_id'])):
        raise ValueError('Native source projection borrowed a noncurrent SQLite head')
    sources = projection.get('sources')
    if not isinstance(sources, list) or len({s.get('sourceId') for s in sources}) != len(sources):
        raise ValueError('Native actual source projection is missing or duplicated')
    for source in sources:
        rows = [r for r in step['tables']['capture_sources'] if r['source_id'] == source.get('sourceId') and r['session_id'] == sid]
        if len(rows) != 1:
            raise ValueError('Native source candidate does not belong to the actual current session')
        row = rows[0]
        mapping = {'streamIdentitySha256':'stream_identity_sha256', 'hookName':'hook_name', 'hookCode':'hook_code',
                   'encoding':'encoding', 'address':'hook_address_u64', 'context':'context_u64', 'context2':'subcontext_u64',
                   'identityExtensionsJson':'identity_extensions_json'}
        if any(source.get(k) != row.get(v) for k, v in mapping.items()):
            raise ValueError('Native actual source typed identity differs from SQLite')
        for key in ('address', 'context', 'context2'):
            value = source.get(key)
            if not isinstance(value, str) or not re.fullmatch(r'0|[1-9][0-9]*', value) or int(value) > 2**64-1:
                raise ValueError('Native actual source u64 projection was rounded')
    return call, returned, projection


def native_reopen(before, after):
    native_same_database_owner(before, after)
    if native_profile(before) != native_profile(after) or before['context'] != after['context']:
        raise ValueError('Native normal reopen changed original profile/session/operation/set authority')
    for key in ('sessions', 'source_selection_sets', 'source_selection_members', 'source_selections'):
        if before['tables'][key] != after['tables'][key]:
            raise ValueError('Native normal reopen committed or changed original selection')
    actions = after.get('native_actions', [])
    if [a.get('id') for a in actions] != ['normal_show_main', 'open_current_sources']:
        raise ValueError('Native normal showMain/source-sheet reopen actions missing')
    cursor = before['os']['observed_at_ms']
    for action in actions:
        if not cursor <= action['started_at_ms'] <= action['finished_at_ms'] <= after['os']['observed_at_ms']:
            raise ValueError('Native reopen actual command interval/order differs')
        cursor = action['finished_at_ms']
    if after['main'].get('on_screen') is not True or before['main']['window_id'] != after['main']['window_id']:
        raise ValueError('Normal Native reopen did not restore that actual Main')
    old_sequence = before['raw']['journal_end_sequence']
    delta = [e for e in after['journal'] if e['sequence'] > old_sequence]
    if any(e['action'] in {'persist_commit', 'show_hud_completed', 'hide_main_completed'}
           or e.get('method') in {'associate', 'beginReading', 'retryReadingWindowEffects', 'disconnect'} for e in delta):
        raise ValueError('Read-only Native reopen repeated association/commit/original window effects')
    call, _returned, projection = native_source_projection(after)
    if call['sequence'] <= old_sequence:
        raise ValueError('Reopened source panel reused a cached query instead of actual read')
    return projection


def native_progress_trace(steps):
    by_id = {s['id']: s for s in steps}
    candidate = by_id.get('candidate')
    if candidate is None:
        raise ValueError('Actual Native current source-selection panel absent')
    profile = native_profile(candidate)
    sid = candidate['context']['session_id']
    calls = native_pairs(candidate['journal'], 'associationProgress')
    qualified = []
    for call, returned in calls:
        value = returned.get('response', {})
        if (value.get('sessionId') == sid and value.get('profile') == profile and value.get('superseded') is False
                and value.get('error') is None and call.get('args', {}).get('operationId') == value.get('operationId')
                and call['args'].get('expectedProfileGeneration') == profile['profileGeneration']):
            qualified.append((call, returned, value))
    if not qualified or qualified[-1][2].get('stage') != 'candidates_ready':
        raise ValueError('Native candidate lacks actual current-operation progress facts')
    _call, returned, progress = qualified[-1]
    op = progress['operationId']
    if not any(r.get('operation_id') == op and r.get('session_id') == sid for r in candidate['tables']['capture_operations']):
        raise ValueError('Native progress operation is not the actual current capture operation')
    facts = progress.get('facts', [])
    db_events = {r['event_id']:r for r in candidate['tables']['hook_events'] if r.get('operation_id') == op and r.get('session_id') == sid}
    actual = []
    for fact in facts:
        row = db_events.get(fact.get('eventId'))
        if (row is None or fact.get('sequence') != str(row['event_sequence'])
                or fact.get('observedAtMs') != str(row['observed_at_ms']) or fact.get('code') != row.get('code')
                or fact.get('level') != row.get('level') or fact.get('message') != row.get('message')):
            raise ValueError('Native progress facts differ from actual current Hook ledger')
        actual.append(row)
    if not actual or len({r['event_id'] for r in actual}) != len(actual):
        raise ValueError('Native progress Hook ledger facts missing/duplicated')
    handshake = [r['event_sequence'] for r in actual if r['code'] in {'host_ready','host_response'}]
    connected = [r['event_sequence'] for r in actual if r['code'] in {'capture_connected','process_connected'}]
    source_events = [r['event_sequence'] for r in actual if r['code'] in {'stream_created','source_discovered','text_received'}]
    if not handshake or not connected or not source_events or not min(handshake) < min(connected) < min(source_events):
        raise ValueError('Native fast/wait candidate lacks real ordered handshake/connection/source facts')
    _source_call, source_return, projection = native_source_projection(candidate)
    if not projection['sources'] or '选择正文' not in candidate['text']:
        raise ValueError('Actual Native source IDs/panel are unavailable')
    if any(e.get('method') == 'beginReading' and e['action'] == 'port_call' for e in candidate['journal']):
        raise ValueError('Native stage automatically began reading before explicit confirmation')
    if 'waiting' in by_id:
        waiting = by_id['waiting']
        if native_profile(waiting) != profile or waiting['context']['session_id'] != sid:
            raise ValueError('Native waiting/candidate profile or session changed')
        prior = [(a,b,v) for a,b,v in qualified if b['observed_at_ms'] <= waiting['os']['observed_at_ms']]
        if (not prior or prior[-1][2]['operationId'] != op or prior[-1][2].get('stage') != 'waiting_text'
                or '等待' not in waiting['text'] or '选择正文' in waiting['text']
                or any(s.get('discovered_at_ms', 0) <= waiting['os']['observed_at_ms'] for s in waiting['tables']['capture_sources'])
                or waiting['os']['observed_at_ms'] >= candidate['os']['observed_at_ms']):
            raise ValueError('Waiting-observed trace has no real source-free current wait interval')
        return 'waiting-observed'
    if (tuple(s['id'] for s in steps) != ('candidate',)
            or returned['observed_at_ms'] > candidate['os']['observed_at_ms']
            or source_return['observed_at_ms'] > candidate['os']['observed_at_ms']):
        raise ValueError('Fast-candidates trace has no actual current source-availability cutoff')
    return 'fast-candidates'


def inspect_binding_journal(root, manifest, directory, nonce, source):
    """Actual application-owned intent/commit/effect journal, no verdict fields."""
    import base64
    rows, original_records = [], []
    segments = manifest.get('binding_journal', [])
    if not isinstance(segments, list) or not segments:
        raise ValueError('Actual bounded application binding journal is missing')
    for segment in segments:
        path = within(root, segment, directory)
        raw = path.read_bytes()
        if not raw.endswith(b'\n') or len(raw) > 64 * 1024 * 1024:
            raise ValueError('Binding journal tail is incomplete or unbounded')
        original_records.extend(raw.splitlines(keepends=True))
        if sum(map(len, original_records)) > 64*1024*1024:
            raise ValueError('Combined Native binding journal exceeds its original bounded capacity')
        def unique_object(pairs):
            value = {}
            for key, item in pairs:
                if key in value:
                    raise ValueError('Duplicate binding journal JSON key')
                value[key] = item
            return value
        rows.extend(json.loads(line, object_pairs_hook=unique_object) for line in raw.splitlines())
    if not rows:
        raise ValueError('Actual binding journal is empty')
    first = rows[0]
    if (first.get('kind') != 'binding_journal_begin' or first.get('schema_version') != 1
            or first.get('nonce') != nonce or first.get('pid') != manifest['main_pid']
            or first.get('source_tree_sha256') != tree_sha(source)
            or first.get('profile_id') != manifest['profile_id']):
        raise ValueError('Application binding journal source/PID/profile/nonce differs')
    events, watermark, contexts = [], 0, {}
    actions = {'context_transition', 'intent', 'port_call', 'port_return', 'persist_commit',
               'show_hud_completed', 'hide_main_completed', 'window_effect_failed', 'operation_cancelled'}
    for row in rows[1:]:
        if (row.get('nonce') != nonce or row.get('pid') != manifest['main_pid']
                or row.get('profile_id') != manifest['profile_id']
                or row.get('boot_sec') != first['boot_sec'] or row.get('boot_usec') != first['boot_usec']):
            raise ValueError('Binding journal cohort changed')
        if row.get('kind') == 'binding_journal_health':
            if (any(type(row.get(key)) is not int for key in ('dropped', 'invalid', 'write_failures', 'accepted', 'written', 'last_sequence'))
                    or any(row.get(key) != 0 for key in ('dropped', 'invalid', 'write_failures'))
                    or any(row.get(key) != watermark for key in ('accepted', 'written', 'last_sequence'))):
                raise ValueError('Binding journal has loss/unclosed sequence health')
        elif row.get('kind') == 'binding_journal_event':
            if (type(row.get('sequence')) is not int or row['sequence'] != watermark + 1
                    or row.get('action') not in actions or type(row.get('observed_at_ms')) is not int
                    or not manifest['started_at_ms'] <= row['observed_at_ms'] <= manifest['finished_at_ms']):
                raise ValueError('Binding journal event sequence/action/interval differs')
            if type(row.get('correlation_id')) is not str or not row['correlation_id']:
                raise ValueError('Binding journal real opaque correlation identity missing')
            for field in ('session_id', 'operation_id', 'selection_set_revision', 'review_id'):
                if row.get(field) is not None and (type(row[field]) is not str or not row[field]):
                    raise ValueError('Binding journal allocated identity must be exact; unallocated is null')
            context_fields = ('review_id', 'session_id', 'operation_id', 'selection_set_revision',
                              'active_target_identity_sha256', 'candidate_target_identity_sha256')
            context = {key: row.get(key) for key in context_fields}
            if row['action'] == 'context_transition':
                contexts[row['correlation_id']] = context
            elif context != contexts.get(row['correlation_id'], dict.fromkeys(context_fields)):
                raise ValueError('Binding journal context crossed an actual opaque correlation')
            if any(key in row for key in ('milestones', 'expected', 'passed', 'verdict')):
                raise ValueError('Application journal must contain raw effects, not producer verdicts')
            watermark += 1
            events.append(row)
        else:
            raise ValueError('Unregistered binding journal record')
    if rows[-1].get('kind') != 'binding_journal_health' or not watermark:
        raise ValueError('Binding journal has no complete actual event interval')
    raw = b''.join(original_records)
    first = dict(first, __native_archive={'sha256': sha(raw), 'size': str(len(raw)), 'health': rows[-1],
                                        'records_base64': [base64.b64encode(line).decode('ascii') for line in original_records]})
    return first, events


def inspect_native_step(root, step, directory, package, source, nonce, pid, started, finished, facts, bounds, journal,
                        *, case_id=None, known_windows=None):
    """Independent OS/AX/raster + actual app-open SQLite backup, no UI booleans."""
    os_path = within(root, step['os'], directory)
    observed = load(os_path)
    if (observed.get('kind') != 'independent_macos_runtime_observation'
            or observed.get('schema_version') != 1 or observed.get('screen_capture_preflight') is not True
            or observed.get('boot_status') != 0 or type(observed.get('observed_at_ms')) is not int
            or not started <= observed['observed_at_ms'] <= finished):
        raise ValueError('Actual ordered Native OS observation unavailable')
    processes = [row for row in observed['processes'] if row.get('pid') == pid]
    if len(processes) != 1:
        raise ValueError('Native application process is absent/ambiguous')
    process = processes[0]
    if (observed.get('boot_sec') != journal[0]['boot_sec']
            or observed.get('boot_usec') != journal[0]['boot_usec']):
        raise ValueError('Native independent OS boot differs from application journal')
    signature = package['signature_metadata']
    if (process.get('guest_code', {}).get('validity') != 0
            or process['guest_code'].get('cdhash') != signature['cdhash']
            or process['guest_code'].get('identifier') != 'com.lunatranslator.reader'
            or not process.get('start_sec') or not process.get('executable')):
        raise ValueError('Native OS process does not match the inspected signed package')
    executable = Path(process['executable'])
    if not executable.is_file() or executable.is_symlink() or sha(executable.read_bytes()) != package['executable_sha256']:
        raise ValueError('Actual running Native executable differs from sealed package bytes')
    from package_identity import read_signature_metadata
    if read_signature_metadata(executable.parent.parent.parent) != signature:
        raise ValueError('Actual installed Native application resource/code signature differs')
    os_execution = inspect_command(root, step['os_execution'], directory)
    command = os_execution['command']
    observer = within(root, step['observer_binary'], directory, limit=64 * 1024 * 1024)
    compile_record = inspect_command(root, step['observer_compilation'], directory)
    observer_source = 'scripts/quality/runtime_observer.swift'
    if (compile_record['command'] != ['/usr/bin/xcrun', 'swiftc', str(Path(root) / observer_source), '-o', str(observer)]
            or compile_record['cwd'] != '.'
            or step.get('observer_source') != {'path': observer_source, 'sha256': source.get(observer_source)}
            or command[:3] != [str(observer), '--output', str(os_path)]
            or len(command) != 7 or command[3] != '--pids' or command[5] != '--windows'
            or str(pid) not in command[4].split(',')):
        raise ValueError('Native observation was not executed by the exact independent OS observer')
    requested_windows = {int(value) for value in command[6].split(',')}
    known_windows = {} if known_windows is None else known_windows
    selected, hud, capture_role = native_windows(observed, step, case_id, pid, requested_windows, known_windows)
    words = []
    for window in observed['windows']:
        capture = window.get('capture', {})
        for image in ([capture] if capture.get('png') else []) + capture.get('preview_crops', []):
            name = image['png']
            if not isinstance(name, str) or Path(name).name != name:
                raise ValueError('Native screenshot must belong to this exact OS capture directory')
            image_path = path_at(root, (os_path.parent / name).relative_to(root).as_posix())
            expected_image = step.get('images', {}).get(name)
            if expected_image != {'path': image_path.relative_to(root).as_posix(), 'sha256': sha(image_path.read_bytes())}:
                raise ValueError('Original Native image bytes are absent from exact invocation inventory')
            if png_dimensions(image_path.read_bytes()) != (image['width'], image['height']):
                raise ValueError('Original Native screenshot dimensions differ')
        if window.get('window_id') == capture_role:
            if not capture.get('png') or not capture.get('ocr'):
                raise ValueError('Independent Native main screenshot/OCR missing')
            words.extend(row['text'] for row in capture['ocr'])
    words.extend(str(row.get(key, '')) for row in process.get('ax', []) for key in ('title', 'value', 'description'))
    database = within(root, step['database'], directory, limit=512 * 1024 * 1024)
    backup = inspect_command(root, step['database_execution'], directory)
    actual_database = step.get('actual_database')
    if (not isinstance(actual_database, str) or not Path(actual_database).is_absolute()
            or backup['command'] != ['/usr/bin/sqlite3', actual_database, '.backup ' + str(database)]):
        raise ValueError('Native SQLite data is not an actual app database backup command')
    opened = inspect_command(root, step['lsof_execution'], directory)
    if opened['command'] != ['/usr/sbin/lsof', '-nP', '-p', str(pid), '-F', 'pfin']:
        raise ValueError('Native actual app-open database observation missing')
    opened_text = checked_ref(root, opened['stdout'], ignored=True).read_text()
    if 'n' + actual_database not in opened_text.splitlines():
        raise ValueError('Native database was not opened by the inspected app process')
    from runtime_stability_evidence import lsof_entries
    entries = lsof_entries(opened_text)
    if [line for line in opened_text.splitlines() if line.startswith('p')] != ['p'+str(pid)]:
        raise ValueError('Native actual open-file observation belongs to another process')
    if not any(row.get('name') == step['actual_journal_path'] and row.get('inode') == step.get('actual_journal_inode')
               for row in entries):
        raise ValueError('Binding journal was not held open by actual application FD/inode')
    database_inodes = {row.get('inode') for row in entries if row.get('name') == actual_database}
    if len(database_inodes) != 1 or not next(iter(database_inodes)):
        raise ValueError('Native actual app-open database FD/inode is absent or ambiguous')
    # The raw lists must be exact projections of the app's loss-free archived
    # runtime ledger, not values supplied by a test's output JSON.
    active = native_closed_events(journal[1], bounds, observed['observed_at_ms'], step.get('journal_end_sequence'))
    transitions = [row for row in active if row['action'] == 'context_transition']
    if not transitions:
        raise ValueError('Native observation has no actual nullable context/transition journal')
    fields = ('review_id', 'session_id', 'operation_id', 'selection_set_revision',
              'active_target_identity_sha256', 'candidate_target_identity_sha256')
    context = {key: transitions[-1].get(key) for key in fields}
    if step.get('context') != context:
        raise ValueError('Native step used caller future/stale identity instead of actual journal transition')
    intervals = {}
    for key, command_record in (('os', os_execution), ('database', backup), ('lsof', opened)):
        a, b = interval(command_record['started_at'], command_record['finished_at'])
        pair = [int(a.timestamp()*1000), int(b.timestamp()*1000)]
        if not started <= pair[0] <= pair[1] <= finished:
            raise ValueError('Native OS/SQLite/FD command escaped actual fixed writer interval')
        intervals[key] = pair
    for key in ('active_target_identity_sha256', 'candidate_target_identity_sha256'):
        if context[key] is not None and (type(context[key]) is not str or not re.fullmatch(r'[0-9a-f]{64}', context[key])):
            raise ValueError('Native active/candidate target identities must be distinct exact hashes or null')
    if context['selection_set_revision'] is not None and not re.fullmatch(r'[1-9][0-9]*', context['selection_set_revision']):
        raise ValueError('Allocated Native selection set revision is not canonical')
    calls = [row for row in active if row['action'] in {'port_call', 'port_return'}]
    # Existing collector -> service commit -> delivery facts own physical frames.
    # No separate arbitrary frames[] observation can manufacture capture IDs.
    from runtime_stability_evidence import new_frames, EvidenceUnavailable
    frame_snapshot = {'os': observed, 'action': {'game_host_pid': step['game_host_pid']}}
    try:
        pairs = new_frames(facts, frame_snapshot)
        frames = [dict(frame, source_id=collector['source_id'], capture_id=collector['capture_id'])
                  for collector, frame in pairs if collector['window_id'] == step['game_window_id']]
    except EvidenceUnavailable:
        frames = []
    current = [row['fact'] for row in facts if row['observed_at_ms'] <= observed['observed_at_ms']
               and row['fact'].get('kind') == 'projection' and row['fact'].get('stage') == 'server_delivered'
               and row['fact'].get('projection', {}).get('consumer_id') == step['consumer_id']]
    if current and current[-1]['projection'].get('permission') in {'denied', 'revoked'} and not current[-1].get('frame'):
        frames = []
    if (load(within(root, step['calls'], directory)) != calls
            or load(within(root, step['frames'], directory)) != frames):
        raise ValueError('Native call/frame values differ from the actual closed runtime ledger')
    all_tables = {table: native_rows(database, table) for table in
              ('profile_meta', 'sessions', 'capture_operations', 'hook_events', 'capture_sources',
               'source_selection_sets', 'source_selection_members', 'source_selections')}
    tables = {name: rows if name == 'profile_meta' else [row for row in rows if context['session_id'] is not None and row.get('session_id') == context['session_id']]
              for name, rows in all_tables.items()}
    sessions = tables['sessions']
    if sessions and (len(sessions) != 1 or sessions[0].get('target_identity_sha256') != context['active_target_identity_sha256']):
        raise ValueError('Native SQLite session belongs to another current target')
    if native_profile({'tables': tables})['profileId'] != journal[0]['profile_id']:
        raise ValueError('Actual Native SQLite belongs to another journal profile')
    fence_tables = dict(tables)
    if context['session_id'] is None:
        fence_tables['sessions'] = all_tables['sessions']
    fence = inspect_native_journal_fence(root, step, directory, journal, fence_tables, context,
                                        opened_text, pid, started, finished)
    sampling = native_sampling_closure(observed['observed_at_ms'], intervals, journal[1], context, fence)
    actions = []
    for action in step.get('native_actions', []):
        if not isinstance(action, dict) or set(action) != {'id', 'execution'} or action['id'] not in {'normal_show_main', 'open_current_sources'}:
            raise ValueError('Unregistered Native normal reopen action')
        execution = inspect_command(root, action['execution'], directory)
        expected = native_action_command(action['id'], pid)
        if execution['command'] != expected:
            raise ValueError('Native reopen action was not the fixed actual normal menu/UI command')
        a, b = interval(execution['started_at'], execution['finished_at'])
        if not started <= int(a.timestamp()*1000) <= int(b.timestamp()*1000) <= observed['observed_at_ms']:
            raise ValueError('Native normal reopen action is outside its actual OS observation interval')
        actions.append({'id': action['id'], 'started_at_ms': int(a.timestamp()*1000), 'finished_at_ms': int(b.timestamp()*1000), 'execution': action['execution']})
    return {'id': step['id'], 'os': observed, 'process': process, 'main': selected, 'hud': hud,
            'text': ''.join(words), 'calls': calls, 'frames': frames, 'tables': tables,
            'database': database, 'raw': step, 'context': context, 'journal': active, 'native_actions': actions,
            'sampling': sampling, 'database_owner': {'pid': pid, 'path': actual_database,
                'inode': next(iter(database_inodes)), 'journal_path': step['actual_journal_path'],
                'journal_inode': step['actual_journal_inode']}}


def native_semantics(case_id, steps):
    """Native happy-path observations; domain/storage own race/fault matrices."""
    by_id = {step['id']: step for step in steps}
    errors = []
    def need(condition, message):
        if not condition:
            errors.append(case_id + ': ' + message)
    def calls(name, method):
        return [row for row in by_id[name]['calls'] if row.get('action') == 'port_call' and row.get('method') == method]
    def table(name, field):
        return by_id[name]['tables'][field]
    def text(name, value):
        return ''.join(value.split()) in ''.join(by_id[name]['text'].split())
    def unchanged(a, b):
        return all(table(a, key) == table(b, key) for key in ('sessions', 'source_selection_sets', 'source_selection_members'))
    if case_id == 'BJ-01':
        need(unchanged('before', 'selected'), 'choosing a target changed durable connection/selection')
        need(len(calls('selected', 'associate')) == 0, 'target selection injected before confirmation')
        need(len(calls('associated', 'associate')) == 1, 'confirmed association did not execute once')
        need(bool(table('associated', 'hook_events')), 'Hook events absent after Native confirmation')
        need(by_id['selected']['context']['candidate_target_identity_sha256'] is not None
             and by_id['associated']['context']['active_target_identity_sha256'] == by_id['selected']['context']['candidate_target_identity_sha256'],
             'confirmed active session does not belong to the independently selected candidate')
    elif case_id == 'BJ-02':
        need(text('confirmation', '选择对应的游戏窗口') and text('confirmation', '需要'), 'system confirmation action/reason absent')
        need(not by_id['confirmation']['frames'] and text('confirmation', '尚无成功画面'), 'fabricated first-frame state/time')
    elif case_id == 'BJ-03':
        a, b = by_id['candidate']['frames'], by_id['associated']['frames']
        need(bool(a) and bool(b), 'candidate/associated real frame ledger absent')
        if a and b:
            need(a[-1]['capture_id'] == b[-1]['capture_id'] and a[-1]['source_id'] == b[-1]['source_id'], 'association replaced original capture/filter')
            need((b[-1]['lease_id'], b[-1]['generation']) != (a[-1]['lease_id'], a[-1]['generation']),
                 'candidate-to-session consumer generation did not change')
        need(not calls('associated', 'selectWindowPreview'), 'normal transfer reopened picker')
    elif case_id == 'BJ-04':
        for name in ('before', 'after'):
            need(not calls(name, 'selectWindowPreview') and not calls(name, 'associate'), 'no-intent/terminal path invoked picker or association')
        need(unchanged('before', 'after'), 'idle operation changed connection')
        need(len(calls('cancel_intent', 'selectWindowPreview')) == 1 and len(calls('cancelled', 'selectWindowPreview')) == 1,
             'cancelled picker was not opened exactly once by explicit intent')
        need(len(calls('timeout_intent', 'selectWindowPreview')) == 2 and len(calls('timeout', 'selectWindowPreview')) == 2,
             'timed-out picker reopened without a new explicit intent')
        for name, code in [('cancelled', 'preview_selection_cancelled'), ('timeout', 'preview_selection_timeout')]:
            need(any(row.get('action') == 'port_return' and row.get('method') == 'selectWindowPreview'
                     and row.get('response', {}).get('errorCode') == code for row in by_id[name]['calls']),
                 'picker actual terminal response absent: ' + code)
    elif case_id == 'BJ-05':
        need(text('reviewed', '复用') and not text('reviewed', '切换并接入'), 'same-target Native review does not reuse')
        need(unchanged('before', 'after'), 'same-target reuse replaced durable session/selection')
        need(by_id['reviewed']['context']['active_target_identity_sha256'] is not None
             and by_id['reviewed']['context']['active_target_identity_sha256'] == by_id['reviewed']['context']['candidate_target_identity_sha256'],
             'reuse review is not actually the same active/candidate target')
        old = {(row['session_id'], row['event_sequence']) for row in table('before', 'hook_events')}
        new = [row for row in table('after', 'hook_events') if (row['session_id'], row['event_sequence']) not in old]
        need(not any(row.get('code') in {'host_ready', 'process_connected', 'hook_inserted'} for row in new),
             'same-target reuse repeated Host/Hook connection')
    elif case_id == 'BJ-06':
        try:
            native_progress_trace(steps)
        except (ValueError, KeyError, TypeError) as error:
            need(False, str(error))
    elif case_id == 'BJ-07':
        for name in ('closed', 'cancelled'):
            need(unchanged('before', name), 'closing/cancelling candidate mutated active durable session')
            need(not calls(name, 'disconnect'), 'closing candidate disconnected existing Hook')
            old, new = by_id['before']['frames'], by_id[name]['frames']
            need(bool(old) and bool(new) and old[-1]['capture_id'] == new[-1]['capture_id'], 'candidate closing stopped/replaced existing capture')
    elif case_id == 'BJ-08':
        need(unchanged('before', 'draft') and not calls('draft', 'beginReading'), 'Native checkbox draft committed')
        need(text('draft', '开始阅读'), 'explicit Native begin-reading control absent')
        need(len(calls('committed', 'beginReading')) == 1, 'Native commit did not execute exactly once')
        context = by_id['committed']['context']
        rows = [row for row in table('committed', 'source_selection_sets')
                if row['operation_id'] == context['operation_id']
                and str(row['selection_set_revision']) == context['selection_set_revision']]
        need(len(rows) == 1 and rows[0]['state'] == 'sealed' and rows[0]['member_count'] >= 2,
             'Native confirmed current multi-source set was not atomically persisted')
        try:
            native_success_outcome(by_id['before'], by_id['committed'])
        except (ValueError, KeyError, TypeError) as error:
            need(False, str(error))
    elif case_id == 'BJ-09':
        saved_before = [row for row in table('before', 'source_selection_sets') if row['persistence_kind'] == 'saved_snapshot']
        saved_temporary = [row for row in table('temporary', 'source_selection_sets') if row['persistence_kind'] == 'saved_snapshot']
        need(bool(saved_before) and saved_before == saved_temporary, 'temporary selection changed or never exercised a nonempty previous saved matcher')
        try:
            baseline = native_selection_receipt(by_id['before'])
            if baseline['row']['persistence_kind'] != 'saved_snapshot':
                raise ValueError('Native remember test has no actual earlier normal saved baseline provenance')
            for row in saved_before:
                native_saved_descriptor(by_id['before'], row)
            temporary = native_success_outcome(by_id['before'], by_id['temporary'])
            if temporary['row']['persistence_kind'] != 'temporary':
                raise ValueError('Native temporary intent was not actually temporary')
            projection = native_reopen(by_id['temporary'], by_id['save_intent'])
            if projection.get('rememberAllowed') is not True or not text('save_intent', '记住选择'):
                raise ValueError('Owned-game actual remember intent/control unavailable')
            saved = native_success_outcome(by_id['save_intent'], by_id['saved'])
            if (saved['call']['args']['remember'] is not True or saved['row']['persistence_kind'] != 'saved_snapshot'
                    or saved['call']['observed_at_ms'] <= by_id['save_intent']['os']['observed_at_ms']):
                raise ValueError('Saved result has no actual later explicit remember=true intent')
            native_saved_descriptor(by_id['saved'], saved['row'])
            reopened = native_reopen(by_id['saved'], by_id['saved_reopened'])
            if reopened.get('rememberAllowed') is not True or reopened.get('current') != saved['receipt']['selection']:
                raise ValueError('Reopened saved policy is not actual persisted current set')
            # A reopened checkbox is a new draft and legitimately resets false.
            unowned = by_id['unowned']
            _a, _b, source = native_source_projection(unowned)
            if (source.get('rememberAllowed') is not False or len(unowned['tables']['sessions']) != 1
                    or unowned['tables']['sessions'][0].get('game_id') is not None):
                raise ValueError('Unowned remember used borrowed game authority')
            for a, b in native_pairs(unowned['journal'], 'beginReading'):
                if a.get('args', {}).get('sessionId') == unowned['context']['session_id'] and a['args'].get('remember') is True:
                    if b.get('response', {}).get('errorCode') != 'remember_requires_game':
                        raise ValueError('Unowned remember silently downgraded instead of typed rejection')
        except (ValueError, KeyError, TypeError) as error:
            need(False, str(error))
    elif case_id == 'BJ-13':
        try:
            native_success_outcome(by_id['before'], by_id['completed'])
        except (ValueError, KeyError, TypeError) as error:
            need(False, str(error))
    elif case_id == 'BJ-14':
        for name in NATIVE_STEPS[case_id]:
            need(text(name, '选择正在运行的游戏') and text(name, '运行窗口预览'), 'Native target/preview regions inaccessible at approved size')
        # The registered visual dispatcher is separately inspected by caller;
        # arbitrary difference percentages cannot authorize a reference.
    elif case_id == 'BJ-15':
        a, b, failed, revoked = [by_id[name]['frames'] for name in NATIVE_STEPS[case_id]]
        need(bool(a) and bool(b) and bool(failed), 'real first/new/failed frame metadata absent')
        if a and b and failed:
            need(int(b[-1]['captured_at_ms']) > int(a[-1]['captured_at_ms']), 'new frame did not advance actual capture time')
            need(failed[-1]['captured_at_ms'] == b[-1]['captured_at_ms'], 'failure changed retained successful capture time')
        need(not revoked and text('revoked', '权限'), 'permission revocation retained a game frame')
        need(bool(table('revoked', 'hook_events')), 'permission revocation stopped independent Hook evidence')
    return errors


def inspect_native_visual(root, comparison, candidate_step, source, package):
    """Current-source independent Native reference, original whole-client PNGs.

    A fresh non-author source/reference review is required. It cannot approve
    product code or revise an existing reference/threshold in place.
    """
    from binding_phase_guard import approved_design_basis, policy_at
    from current_general_visual_recompute import pixels
    from current_general_native_display import validate_probe, primary_environment, require_same_environment
    review_path = checked_ref(root, comparison['review'], ignored=True)
    review = load(review_path)
    policy, _ = policy_at(root)
    if (review.get('kind') != 'independent-binding-native-reference-review/v1'
            or review.get('schema_version') != 1 or review.get('status') != 'approved'
            or review.get('independent_non_author_review') is not True
            or review.get('author_ids') != policy['authors']
            or not isinstance(review.get('reviewer_id'), str) or review['reviewer_id'] in policy['authors']
            or review.get('blocking_findings') != []):
        raise ValueError('Independent non-author binding Native reference review required')
    packet_path = checked_ref(root, review['reference_packet'], ignored=True)
    packet = load(packet_path)
    if (packet.get('kind') != 'approved-source-binding-native-reference/v1'
            or packet.get('schema_version') != 1 or packet.get('case_id') != 'BJ-14'
            or packet.get('design_basis_sha256') != approved_design_basis(root)
            or packet.get('product_acceptance_promoted') is not False
            or packet.get('source_kind') != 'approved_html_in_native_webkit'
            or packet.get('variants') is None):
        raise ValueError('Native reference must originate from the exact complete approved design source')
    variant = candidate_step['id']
    if set(packet['variants']) != set(NATIVE_STEPS['BJ-14']):
        raise ValueError('Exact default/narrow Native reference inventory required')
    reference = packet['variants'][variant]
    context = candidate_step['raw'].get('visual_context')
    if (not isinstance(context, dict) or set(context) != {'viewport', 'device_scale_factor', 'font', 'zoom', 'theme', 'state_sha256', 'selector', 'scope'}
            or context.get('scope') != 'whole_client' or context.get('selector') != '#main-window'
            or reference.get('context') != context):
        raise ValueError('Native reference/candidate state, font, viewport, zoom or scope differs')
    if context['viewport'] != ([1040, 720] if variant == 'dialog_default' else [760, 560]):
        raise ValueError('Native binding viewport is not the approved C1 size')
    entry = reference.get('rendered_entry')
    expected_entry = 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html'
    if entry != {'path': expected_entry, 'sha256': packet['design_basis_sha256'][expected_entry]}:
        raise ValueError('Native reference loaded a different design document')
    # Compile/run a fixed reference producer in the production Native test
    # target; the reviewed source must actually load the approved HTML, not the
    # candidate product page. Its original commands/DOM/OS are retained.
    spec = dict(TRANSPORT_PRODUCER, test_name='bj_14_native_reference')
    command = inspect_test_writer(root, reference, packet_path.parent, spec, source)
    producer_binary = within(root, reference['binary'], packet_path.parent, limit=256 * 1024 * 1024)
    if command['command'] != [str(producer_binary), '--ignored', '--exact', 'bj_14_native_reference', '--nocapture']:
        raise ValueError('Actual registered independent Native design producer command missing')
    compile_record = inspect_command(root, reference['compilation'], packet_path.parent)
    if compile_record['command'] != ['cargo', 'test', '--locked', '--offline', '-p', 'app-infra', '--test', 'binding_journey', '--no-run', '--message-format=json']:
        raise ValueError('Native reference producer compile command differs')
    producer_source = 'crates/app-infra/tests/binding_journey.rs'
    if reference.get('producer_source') != {'path': producer_source, 'sha256': source.get(producer_source)}:
        raise ValueError('Native reference generator is not the independently reviewed current source')
    text = checked_ref(root, command['stdout'], ignored=True).read_text()
    if not exact_test_outcome(text, 'bj_14_native_reference', command['exit_code']):
        raise ValueError('Independent Native reference producer did not actually execute')
    dom = load(within(root, reference['dom'], packet_path.parent))
    if (dom.get('document_path') != str(Path(root) / expected_entry)
            or dom.get('design_basis_sha256') != packet['design_basis_sha256']
            or dom.get('context') != context):
        raise ValueError('Original Native reference DOM loaded a different source/context')
    # Display environment is verified from original public API probes, rather
    # than equating CSS viewport with the physical monitor used for capture.
    environments = []
    for side in (reference, candidate_step['raw']):
        probes = side.get('display_probes')
        if not isinstance(probes, dict) or set(probes) != {'pre_launch', 'before_capture', 'after_capture'}:
            raise ValueError('Original complete Native display probe interval missing')
        values = []
        for phase, value in probes.items():
            original = load(checked_ref(root, value, ignored=True))
            validate_probe(original, side['display_nonce'], phase)
            values.append(primary_environment(original))
        for value in values[1:]:
            require_same_environment(values[0], value)
        environments.append(values[0])
    require_same_environment(*environments)
    reference_png = within(root, reference['png'], packet_path.parent)
    reference_os = load(within(root, reference['os'], packet_path.parent))
    windows = [row for row in reference_os.get('windows', []) if row.get('window_id') == reference.get('window_id')]
    if (reference_os.get('kind') != 'independent_macos_runtime_observation' or len(windows) != 1
            or windows[0].get('pid') != reference.get('renderer_pid') or windows[0].get('on_screen') is not True
            or windows[0].get('capture', {}).get('png') != reference_png.name):
        raise ValueError('Reference PNG is not the original independent Native window capture')
    candidate = candidate_step['main']['capture']
    candidate_png = checked_ref(root, candidate_step['raw']['os'], ignored=True).parent / candidate['png']
    original = pixels(reference_png, candidate_png)
    # Existing exact baseline remains exact. Any future user disposition must
    # have its own reviewed scope; a producer cannot invent a relaxed threshold.
    if original['different_rgba_pixels'] != 0:
        raise ValueError('Binding Native original whole-client PNG differs from independent approved-source reference')
    return original


def inspect_native_node(root, reference, nonce, source, python_results, directory):
    """Node independently reads original refs/SQLite; stdin contains no Py verdict."""
    import secrets
    import shutil
    import subprocess
    import time
    root = Path(root).resolve()
    node = shutil.which('node')
    if not node:
        raise ValueError('Independent Node Native consumer is unavailable')
    runner = Path(root) / RUNNER
    if sha(runner.read_bytes()) != source.get(RUNNER):
        raise ValueError('Independent Node Native consumer source differs from exact run source')
    script = ('import fs from "node:fs";import {pathToFileURL} from "node:url";'
              'const p=JSON.parse(fs.readFileSync(0,"utf8"));'
              'const {inspectNativeRawCollection}=await import(pathToFileURL(p.runner).href);'
              'delete p.runner;process.stdout.write(JSON.stringify(await inspectNativeRawCollection(p)));')
    command = [node, '--input-type=module', '--eval', script]
    payload = {'runner': str(runner.resolve()), 'root': str(Path(root).resolve()), 'reference': reference,
               'nonce': nonce, 'source_tree_sha256': tree_sha(source)}
    started = time.time_ns()//1_000_000
    process = subprocess.run(command, input=json.dumps(payload).encode(), capture_output=True,
                             cwd=root, timeout=120)
    finished = time.time_ns()//1_000_000
    output = Path(directory) / ('independent-node-native-inspection-' + secrets.token_hex(8))
    output.mkdir(exist_ok=False)
    for name, raw in [('stdin.json', json.dumps(payload).encode()), ('stdout.json', process.stdout), ('stderr.txt', process.stderr)]:
        (output/name).write_bytes(raw)
    record = {'kind': 'independent-node-native-consumer-execution/v1', 'command': command, 'cwd': '.',
              'started_at_ms': started, 'finished_at_ms': finished, 'exit_code': process.returncode,
              'node_binary': {'path': str(Path(node).resolve()), 'sha256': sha(Path(node).read_bytes())},
              'runner': {'path': RUNNER, 'sha256': source[RUNNER]}, 'source_tree_sha256': tree_sha(source),
              'stdin': ref(root,(output/'stdin.json').relative_to(root).as_posix()),
              'stdout': ref(root,(output/'stdout.json').relative_to(root).as_posix()),
              'stderr': ref(root,(output/'stderr.txt').relative_to(root).as_posix()),
              'product_or_native_execution_performed': False}
    (output/'execution.json').write_text(json.dumps(record, sort_keys=True, indent=2)+'\n')
    if process.returncode:
        raise ValueError('Independent Node Native original-data inspection failed: ' + process.stderr.decode(errors='replace')[:1000])
    observed = strict_json_text(process.stdout.decode())
    actual = [{key: row.get(key) for key in ('case_id','status')} for row in observed.get('results', [])]
    expected = [{key: row[key] for key in ('case_id','status')} for row in python_results
                if row['layer'] == 'native' and row['case_id'] in {'BJ-06','BJ-08','BJ-09','BJ-13'}]
    if observed.get('kind') != 'independent-node-native-observation-inspection/v1' or actual != expected:
        raise ValueError('Node/Python Native branch/role/authority/DB judgments disagree')
    return ref(root, (output/'execution.json').relative_to(root).as_posix())


def inspect_native_collection(root, reference, nonce, source):
    """Fixed ignored Rust writer with case-specific original OS/DB/frame facts."""
    path = checked_ref(root, reference, ignored=True)
    directory, manifest = path.parent, load(path)
    if (manifest.get('kind') != 'binding-journey-native-collection/v1' or manifest.get('schema_version') != 1
            or manifest.get('nonce') != nonce or manifest.get('source_tree_sha256') != tree_sha(source)
            or manifest.get('source_map') != source or manifest.get('fixture_substitution') is not False):
        raise ValueError('Native binding collection source/nonce/origin differs')
    from package_identity import inspect_package
    package = inspect_package(root, checked_ref(root, manifest['artifact'], ignored=True), tree_sha(source),
                              directory / 'independent-package-inspection')
    from runtime_stability_evidence import inspect_trace
    facts, _health = inspect_trace(directory, manifest, nonce, manifest['main_pid'], manifest['started_at_ms'])
    journal = inspect_binding_journal(root, manifest, directory, nonce, source)
    contract = load(Path(root) / PLAN)['resume_epoch']['observation_contracts']
    if (contract['native_reading_completion']['native_steps'] != list(NATIVE_STEPS['BJ-13'])
            or contract['native_success_roles']['fixed_registry']['native_case_step_sequences'] !=
            {key: list(NATIVE_STEPS[key]) for key in ('BJ-08', 'BJ-09')}
            or {key: tuple(value['steps']) for key, value in contract['native_progress_traces']['fixed_registry']['traces'].items()} != NATIVE_PROGRESS_TRACES):
        raise ValueError('Native fixed source registry differs from frozen P1 observation contract')
    results = []
    found = set()
    for row in manifest['cases']:
        case = row.get('case_id')
        if case not in NATIVE_STEPS or case in found:
            raise ValueError('Exact registered Native case/step inventory required')
        native_inventory(case, row.get('steps', []))
        if any(key in row for key in ('branch', 'milestones', 'allow_main_absent', 'role')):
            raise ValueError('Native case cannot self-authorize branch/role/milestones')
        found.add(case)
        bounds = row.get('journal_range')
        if (not isinstance(bounds, dict) or set(bounds) != {'start_sequence', 'end_sequence'}
                or any(type(value) is not int for value in bounds.values())
                or not 1 <= bounds['start_sequence'] <= bounds['end_sequence'] <= len(journal[1])):
            raise ValueError('Native case needs an actual bounded app journal interval')
        producer = row['producer']
        expected = dict(TRANSPORT_PRODUCER, test_name=case.lower().replace('-', '_') + '_native')
        expected['source_sha256'] = source.get(expected['source'])
        if producer != expected:
            raise ValueError('Native case did not use fixed production ignored test writer')
        inspect_test_writer(root, row, directory, expected, source)
        # The same strict compilation/list/exact-command chain as the DTO writer,
        # but Native observations are read-only recomputed and never re-executed.
        command = inspect_command(root, row['execution'], directory)
        binary = within(root, row['binary'], directory, limit=256 * 1024 * 1024)
        if command['command'] != [str(binary), '--ignored', '--exact', producer['test_name'], '--nocapture']:
            raise ValueError('Exact actual Native producer invocation missing')
        if (command.get('environment', {}).get('LUNA_BINDING_NONCE') != nonce
                or command.get('environment', {}).get('LUNA_BINDING_SOURCE_TREE') != tree_sha(source)):
            raise ValueError('Actual Native writer execution did not receive the original nonce/source epoch')
        start, end = interval(command['started_at'], command['finished_at'])
        actual_events = [event for event in journal[1] if int(start.timestamp() * 1000) <= event['observed_at_ms'] <= int(end.timestamp() * 1000)]
        if not actual_events or bounds != {'start_sequence': actual_events[0]['sequence'], 'end_sequence': actual_events[-1]['sequence']}:
            raise ValueError('Native case journal bounds do not close the actual complete writer execution interval')
        compiled = inspect_command(root, row['compilation'], directory)
        if compiled['command'] != ['cargo', 'test', '--locked', '--offline', '-p', 'app-infra', '--test', 'binding_journey', '--no-run', '--message-format=json']:
            raise ValueError('Native writer production compile command differs')
        artifacts = [json.loads(line) for line in checked_ref(root, compiled['stdout'], ignored=True).read_text().splitlines() if line.startswith('{')]
        artifacts = [value for value in artifacts if value.get('reason') == 'compiler-artifact' and value.get('target', {}).get('name') == 'binding_journey' and value.get('profile', {}).get('test')]
        if len(artifacts) != 1 or Path(artifacts[0]['target']['src_path']).resolve() != Path(root) / producer['source']:
            raise ValueError('Native compiled writer source differs')
        listing = inspect_command(root, row['inventory'], directory)
        if listing['command'] != [str(binary), '--list', '--ignored', '--format', 'terse'] or checked_ref(root, listing['stdout'], ignored=True).read_text().splitlines().count(producer['test_name'] + ': test') != 1:
            raise ValueError('Native writer was not in exact compiled ignored-test inventory')
        raw = checked_ref(root, command['stdout'], ignored=True).read_text()
        if not exact_test_outcome(raw, producer['test_name'], command['exit_code']):
            raise ValueError('Native writer did not actually execute')
        steps, known_windows = [], {}
        for step in row['steps']:
            inspected = inspect_native_step(root, step, directory, package, source, nonce, manifest['main_pid'],
                                            int(start.timestamp()*1000), int(end.timestamp()*1000), facts, bounds, journal,
                                            case_id=case, known_windows=known_windows)
            if inspected['native_actions'] and (case != 'BJ-09' or step['id'] not in {'save_intent', 'saved_reopened'}):
                raise ValueError('Native normal reopen is only registered at exact BJ09 read-only edges')
            steps.append(inspected)
        identities = [(step['process']['pid'], step['process']['start_sec'], step['process']['start_usec'],
                       step['process']['executable'], step['os']['boot_sec'], step['os']['boot_usec']) for step in steps]
        if len(set(identities)) != 1 or [step['os']['observed_at_ms'] for step in steps] != sorted(step['os']['observed_at_ms'] for step in steps):
            raise ValueError('Native case process/boot/cohort changed between ordered steps')
        errors = native_semantics(case, steps)
        if case == 'BJ-14':
            design_errors = []
            for step in steps:
                comparison = step['raw'].get('independent_visual_comparison')
                if not comparison:
                    design_errors.append('Native layout comparison independent reference is missing for ' + step['id'])
                else:
                    inspect_native_visual(root, comparison, step, source, package)
            results.append({'case_id': 'BJ-14', 'layer': 'design', 'status': 'blocked' if design_errors else 'passed',
                            'errors': design_errors, 'execution': reference})
        results.append({'case_id': case, 'layer': 'native', 'status': 'failed' if errors else 'passed',
                        'errors': errors, 'execution': row['execution']})
    node_inspection = inspect_native_node(root, reference, nonce, source, results, directory)
    for result in results:
        if result['layer'] == 'native' and result['case_id'] in {'BJ-06','BJ-08','BJ-09','BJ-13'}:
            result['independent_node_inspection'] = node_inspection
    return results


def inspect_native_obligations(root, reference, nonce, source):
    """Native groups remain blocked without actual fixed signed-package writers."""
    groups = {'EX-NATIVE-SUCCESS-ROLES': ('BJ-08', 'BJ-09'),
              'EX-NATIVE-PROGRESS-TRACES': ('BJ-06',),
              'EX-LIFE-SIGNED-NATIVE': ('BJ-07',)}
    if reference is None:
        return [{'id': identity, 'status': 'blocked', 'checks': [],
                 'errors': ['Actual fixed signed-package Native producer evidence is missing'], 'artifacts': []}
                for identity in groups]
    try:
        results = inspect_native_collection(root, reference, nonce, source)
    except (ValueError, KeyError, TypeError, OSError, sqlite3.Error) as error:
        return [{'id': identity, 'status': 'failed', 'checks': [], 'errors': [str(error)], 'artifacts': [reference]}
                for identity in groups]
    native = {row['case_id']: row for row in results if row['layer'] == 'native'}
    output = []
    for identity, cases in groups.items():
        checks = [{'id': case, 'status': native.get(case, {}).get('status', 'blocked')} for case in cases]
        errors = [error for case in cases for error in native.get(case, {}).get('errors', [])]
        status = 'failed' if errors else 'blocked' if any(x['status'] != 'passed' for x in checks) else 'passed'
        if identity == 'EX-LIFE-SIGNED-NATIVE' and status == 'passed':
            status = 'blocked'
            errors.append('Native BJ07 does not discharge separate same-package four-game/BJ16 and real service obligations')
        output.append({'id': identity, 'status': status, 'checks': checks, 'errors': errors, 'artifacts': [reference]})
    return output


def inspect_live_service(root, reference, nonce, source):
    """Real final-package four-game translation chain, never SR6 inference."""
    path = checked_ref(root, reference, ignored=True)
    manifest, directory = load(path), path.parent
    if (manifest.get('kind') != 'binding-journey-live-service/v1' or manifest.get('schema_version') != 1
            or manifest.get('nonce') != nonce or manifest.get('source_tree_sha256') != tree_sha(source)
            or manifest.get('fixture_substitution') is not False
            or [row.get('game_id') for row in manifest.get('games', [])] != ['sr', 'mo', 'never7', 'sakura10th']):
        raise ValueError('Exact genuine current-package four-game service cohort required')
    runtime_path = checked_ref(root, manifest['runtime_collection'], ignored=True)
    runtime = load(runtime_path)
    from runtime_stability_evidence import validate_run, session_for, inspect_body, visible_text
    inspected = validate_run(root, runtime_path.parent, nonce, tree_sha(source))
    if (inspected['status'] != 'passed' or inspected['errors'] or inspected['main_pid'] != manifest.get('main_pid')
            or inspected['artifact_sha256'] != manifest.get('artifact_sha256')):
        raise ValueError('Translation evidence differs from the genuinely inspected package/process/game cohort')
    recorder = manifest.get('wire_recorder')
    if not isinstance(recorder, dict):
        raise ValueError('A real transparent recording proxy producer is required; wire JSON flags are not evidence')
    spec = dict(TRANSPORT_PRODUCER, test_name='bj_service_wire_capture')
    proxy_execution = inspect_test_writer(root, recorder, directory, spec, source)
    proxy_pid = proxy_execution.get('pid')
    environment = proxy_execution.get('environment', {})
    if (type(proxy_pid) is not int or proxy_pid < 1 or environment.get('LUNA_BINDING_NONCE') != nonce
            or environment.get('LUNA_BINDING_SOURCE_TREE') != tree_sha(source)
            or not isinstance(environment.get('LUNA_BINDING_UPSTREAM_ENDPOINT'), str)
            or environment.get('LUNA_BINDING_MODE') != 'recording_proxy'):
        raise ValueError('Actual recording proxy PID/upstream/nonce/source environment differs')
    records_path = within(root, recorder['records'], directory)
    raw_records = records_path.read_bytes()
    if not raw_records.endswith(b'\n') or len(raw_records) > 64 * 1024 * 1024:
        raise ValueError('Original network recording journal incomplete or unbounded')
    wire_records = [strict_json_text(line) for line in raw_records.decode().splitlines()]
    if any(row.get('nonce') != nonce or row.get('proxy_pid') != proxy_pid or row.get('sequence') != number
           for number, row in enumerate(wire_records, 1)):
        raise ValueError('Actual network recording sequence/producer differs')
    from runtime_stability import load_inputs
    inputs = load_inputs(runtime_path.parent / 'inputs.json', root)
    runtime_steps = {row['step_id']: row for row in runtime['steps']}
    for item, game in zip(manifest['games'], inputs['games']):
        if item.get('source_language') != ('zh-CN' if item['game_id'] == 'sr' else 'ja'):
            raise ValueError('SR Chinese cannot claim Japanese service quality')
        step_id = item['game_id'] + '/reading-5'
        record = runtime_steps[step_id]
        slot = runtime_path.parent / record['directory']
        snapshot = {name: load(slot / (name + '.json')) for name in ('os', 'database', 'identity')}
        snapshot.update(action=load(slot / 'barrier-action.json'), pid=manifest['main_pid'])
        session = session_for(snapshot, game)
        body = inspect_body(session)
        if not body:
            raise ValueError('Real service has no actual Hook body input')
        database = within(root, item['database'], directory, limit=512 * 1024 * 1024)
        backup = inspect_command(root, item['database_execution'], directory)
        if backup['command'] != ['/usr/bin/sqlite3', inputs['profile_database'], '.backup ' + str(database)]:
            raise ValueError('Live translation SQLite must be an actual cohort database backup')
        connection = sqlite3.connect(database.as_uri() + '?mode=ro&immutable=1', uri=True)
        connection.row_factory = sqlite3.Row
        try:
            def one(sql, args):
                rows = list(connection.execute(sql, args))
                if len(rows) != 1:
                    raise ValueError('Exact real translation identity/version row missing or ambiguous')
                return dict(rows[0])
            result = one('select * from translations where translation_id=?', (item['translation_id'],))
            task = one('select * from request_tasks where task_id=?', (result['task_id'],))
            attempt = one('select * from request_attempts where attempt_id=? and task_id=?', (result['attempt_id'], task['task_id']))
            target = one('select * from task_execution_targets where task_id=?', (task['task_id'],))
            actual = one('select * from attempt_execution_targets where attempt_id=? and task_id=?', (attempt['attempt_id'], task['task_id']))
            fixed = one('select * from reading_unit_revisions where unit_id=? and text_revision=? and session_id=?',
                        (task['unit_id'], task['text_revision'], task['session_id']))
            if (task['purpose'] != 'translation' or task['state'] != 'completed' or attempt['state'] != 'completed'
                    or attempt['dispatch_kind'] == 'cache' or task['session_id'] != session['session']['session_id']
                    or not any(row['unit_id'] == fixed['unit_id'] and row['text_revision'] == fixed['text_revision']
                               and row['source_sha256'] == fixed['source_sha256'] for row in body)
                    or not result['translated_text'].strip() or result['target_language'] != 'zh-CN'
                    or not runtime['started_at_ms'] <= task['created_at_ms'] <= attempt['started_at_ms']
                    <= attempt['finished_at_ms'] <= result['created_at_ms'] <= runtime['observation_completed_at_ms']):
                raise ValueError('Real task/attempt/translation body/language/interval chain differs')
            for row in (attempt, target, actual):
                if row['config_revision_id'] != task['config_revision_id'] or row['binding_id'] != task['binding_id']:
                    raise ValueError('Actual service executed a different immutable model/binding version')
            if actual['target_kind'] != target['target_kind'] or actual['engine_id'] != attempt['actual_engine_id']:
                raise ValueError('Actual attempt engine target differs from the real task')
            if actual['target_kind'] != 'http':
                # Real native/local-worker protocols need their own actual wire
                # result proof. No HTTP model or cache result substitutes them.
                raise ValueError('Live service proof currently requires an actual HTTP request/response capture')
            model = one('select * from models where revision_id=? and model_id=? and connection_id=?',
                        (task['config_revision_id'], actual['http_model_id'], actual['http_connection_id']))
            service = one('select * from connections where revision_id=? and connection_id=?',
                          (task['config_revision_id'], actual['http_connection_id']))
            exchange = load(within(root, item['provider_exchange'], directory))
            if exchange not in wire_records or wire_records.count(exchange) != 1:
                raise ValueError('Service exchange was not an original actual proxy network record')
            if (exchange.get('kind') != 'actual-service-wire-exchange/v1'
                    or exchange.get('nonce') != nonce or exchange.get('main_pid') != manifest['main_pid']
                    or exchange.get('task_id') != task['task_id'] or exchange.get('attempt_id') != attempt['attempt_id']
                    or exchange.get('config_revision_id') != task['config_revision_id']
                    or exchange.get('binding_id') != task['binding_id'] or exchange.get('actual_model_id') != model['actual_model_id']
                    or exchange.get('endpoint') != service['endpoint'] or exchange.get('protocol') != service['protocol']
                    or exchange.get('fixture_substitution') is not False
                    or not 200 <= exchange.get('http_status', 0) < 300):
                raise ValueError('Actual provider exchange does not match immutable executed service/model identity')
            if (exchange.get('upstream_endpoint') != environment['LUNA_BINDING_UPSTREAM_ENDPOINT']
                    or exchange.get('request_body_sha256') != exchange['request_body']['sha256']
                    or exchange.get('response_body_sha256') != exchange['response_body']['sha256']):
                raise ValueError('Actual upstream/wire body byte identities differ')
            if (type(exchange.get('request_received_at_ms')) is not int or type(exchange.get('response_finished_at_ms')) is not int
                    or not attempt['started_at_ms'] <= exchange['request_received_at_ms']
                    <= exchange['response_finished_at_ms'] <= attempt['finished_at_ms']):
                raise ValueError('Actual wire request/response lies outside the exact real attempt interval')
            sockets = exchange.get('sockets', {})
            if set(sockets) != {'client', 'proxy_downstream', 'proxy_upstream'}:
                raise ValueError('Actual accepted client/downstream/upstream socket tuples required')
            client_parts = sockets['client'].split('->')
            if len(client_parts) != 2 or sockets['proxy_downstream'] != '->'.join(reversed(client_parts)):
                raise ValueError('Luna and accepted proxy socket endpoints do not identify the same TCP connection')
            if client_parts[1] != exchange['listen_endpoint'] or sockets['proxy_upstream'].split('->')[-1] != exchange['upstream_peer']:
                raise ValueError('Recorded connection tuple differs from actual listener/upstream')
            for owner, pid in [('client', manifest['main_pid']), ('proxy', proxy_pid)]:
                observed = inspect_command(root, item[owner + '_connections'], directory)
                if observed['command'] != ['/usr/sbin/lsof', '-nP', '-a', '-p', str(pid), '-iTCP', '-F', 'pfinT']:
                    raise ValueError('Actual client/proxy process network connection observation missing')
                connections = checked_ref(root, observed['stdout'], ignored=True).read_text()
                records, current = [], {}
                for line in connections.splitlines():
                    if line.startswith('f'):
                        if current:
                            records.append(current)
                        current = {'fd': line[1:]}
                    elif line.startswith('n'):
                        current['socket'] = line[1:]
                    elif line.startswith('TST='):
                        current['state'] = line[4:]
                if current:
                    records.append(current)
                expected_sockets = [sockets['client']] if owner == 'client' else [sockets['proxy_downstream'], sockets['proxy_upstream']]
                if any(not any(row.get('socket') == expected_socket and row.get('state') == 'ESTABLISHED' for row in records)
                       for expected_socket in expected_sockets):
                    raise ValueError('Actual established Luna/proxy/upstream TCP four-tuples differ from recorded request')
                start_observed, end_observed = interval(observed['started_at'], observed['finished_at'])
                if not exchange['request_received_at_ms'] <= int(start_observed.timestamp() * 1000) <= int(end_observed.timestamp() * 1000) <= exchange['response_finished_at_ms']:
                    raise ValueError('Independent established-connection capture is outside the actual in-flight request')
            request_body = load(within(root, exchange['request_body'], directory))
            response_path = within(root, exchange['response_body'], directory)
            response_raw = response_path.read_text()
            try:
                response_body = strict_json_text(response_raw)
            except ValueError:
                data = [line[5:].strip() for line in response_raw.splitlines() if line.startswith('data:')]
                if not data or data[-1] != '[DONE]':
                    raise ValueError('Original SSE response lacks complete actual protocol terminator')
                response_body = {'events': [strict_json_text(value) for value in data[:-1]]}
            if fixed['source_text'] not in json.dumps(request_body, ensure_ascii=False):
                raise ValueError('Actual wire request does not contain the fixed real game sentence')
            if 'translation_pointers' in exchange:
                parts = [pointer(response_body, value) for value in exchange['translation_pointers']]
                if any(type(value) is not str for value in parts):
                    raise ValueError('Actual streamed translation fragment is not a string')
                translated = ''.join(parts)
            else:
                translated = pointer(response_body, exchange['translation_pointer'])
            if type(translated) is not str or translated != result['translated_text']:
                raise ValueError('Actual wire result does not equal the durable translation')
            if model['actual_model_id'] is not None and exchange.get('model_request_pointer') is not None:
                if pointer(request_body, exchange['model_request_pointer']) != model['actual_model_id']:
                    raise ValueError('Actual requested model ID differs from frozen execution target')
            # Visible Native result is proved by independently captured OCR in
            # the same final package/PID slot, not the service producer's label.
            visible_text(snapshot, 'main', result['translated_text'])
            visible_text(snapshot, 'hud', result['translated_text'])
        finally:
            connection.close()
    return {'case_id': 'BJ-16', 'layer': 'service', 'status': 'passed',
            'execution': reference, 'main_pid': manifest['main_pid'], 'artifact_sha256': manifest['artifact_sha256']}


def inspect_execution_obligations(root, context, source, bundle, rust, layer_results):
    policy = load(path_at(root, 'openspec/changes/align-approved-binding-journey/phase-policy.json'))
    registry = policy['resume_epoch']['execution_obligations']
    if ([row.get('id') for row in registry.get('obligations', [])] != list(EXACT_EXECUTION_OBLIGATION_IDS)
            or registry.get('original_component_scene_count') != 38
            or registry.get('required_component_scene_count_after_p3') != 40
            or registry.get('all_original_76_definitions_unchanged') is not True):
        raise ValueError('All nine reviewed execution groups and original cumulative obligations must remain registered')
    groups = {identity: {'id': identity, 'status': 'blocked', 'checks': [], 'errors': [], 'artifacts': []}
              for identity in EXACT_EXECUTION_OBLIGATION_IDS}
    for group, scene_id in [('EX-LIB-GLOBAL','BJS-01-library-global-owner'), ('EX-LIB-SCOPED','BJS-01-scoped-reconnect-owner')]:
        matches = [row for row in bundle['scenes']['judgments'] if row['id'] == scene_id]
        if len(matches) != 1: raise ValueError('Required actual library ownership scene was omitted')
        judgment = matches[0]
        if judgment['status'] == 'passed' and judgment.get('library_database', {}).get('status') != 'passed':
            raise ValueError('Library ownership cannot pass without independent actual SQLite join')
        groups[group].update(status=judgment['status'], checks=judgment.get('assertions', []),
                             artifacts=[row['trace'] for row in bundle['report']['scenes'] if row['id'] == scene_id])
    core = rust['core_preview_observations']
    groups['EX-LIFE-CORE'].update(status=core['status'], checks=core['checks'], errors=core['errors'], artifacts=[context['rust_layers']])
    native_units = inspect_fixed_native_units(root, context.get('fixed_native_units'), context['nonce'], source)
    for group in native_units: groups[group['id']] = group
    storage = rust['storage_v5_inspection']
    child = storage.get('child_execution') or {}
    # Old v1 qualification remains valid only for its original scope.
    storage_status = storage['status'] if child.get('kind') == 'schema5-child-execution/v1' else 'blocked'
    if child and (len(child.get('parent_timeline_routes', [])) != 9 or len(child.get('fault_routes', [])) != 4 or len(child.get('child_tests', [])) != 3):
        raise ValueError('Schema5 child/timeline proof lost a fixed parent or fault route')
    groups['EX-STORAGE-TIMELINE'].update(status=storage_status, checks=[{'child_execution': child}] if child else [],
                                       errors=storage.get('errors', []), artifacts=[context['rust_layers']])
    for group in inspect_native_obligations(root, context.get('native_evidence'), context['nonce'], source):
        if group['id'] not in {'EX-NATIVE-SUCCESS-ROLES','EX-NATIVE-PROGRESS-TRACES','EX-LIFE-SIGNED-NATIVE'}:
            raise ValueError('Unregistered Native execution group')
        groups[group['id']] = group
    signed = groups['EX-LIFE-SIGNED-NATIVE']
    native_bj07 = [row for row in layer_results if row['case_id'] == 'BJ-07' and row['layer'] == 'native' and row['status'] == 'passed']
    service = [row for row in layer_results if row['case_id'] == 'BJ-16' and row['layer'] == 'service' and row['status'] == 'passed']
    if native_bj07 and service:
        native_manifest = load(checked_ref(root, context['native_evidence'], ignored=True))
        service_manifest = load(checked_ref(root, context['service_evidence'], ignored=True))
        if ((native_manifest.get('artifact') or {}).get('sha256') != service_manifest.get('artifact_sha256')
                or native_manifest.get('main_pid') != service_manifest.get('main_pid')):
            raise ValueError('Signed lifecycle and four-game translation belong to different packages/process slots')
        signed.update(status='passed', errors=[], checks=signed['checks'] + service,
                      artifacts=signed['artifacts'] + [context['service_evidence']])
    return {'schema_version': 1, 'kind': 'binding-execution-obligation-assessment/v1',
            'registry_sha256': tree_sha(registry), 'groups': [groups[identity] for identity in EXACT_EXECUTION_OBLIGATION_IDS],
            'product_acceptance_promoted': False}


def inspect_execution(root, directory, *, require_current=True, expected_source_tree=None):
    """Rebuild verdict from original invocation, producer, DOM and case traces."""
    root, directory = Path(root).resolve(), Path(directory).resolve()
    result = {'schema_version': 1, 'kind': 'binding-journey-inspection/v1',
              'status': 'failed', 'errors': [], 'validation_errors': [],
              'infrastructure_errors': [], 'semantic_failures': [],
              'eligible_semantic_red': False, 'source_tree_sha256': None,
              'case_count': 0, 'passed': 0, 'failed': 0,
              'product_acceptance_promoted': False, 'native_acceptance_promoted': False}
    try:
        if not directory.is_relative_to(root / '.build') or directory.is_symlink():
            raise ValueError('Ignored actual invocation directory required')
        context = load(directory / 'execution.json')
        if (context.get('kind') != 'binding-journey-invocation/v1'
                or type(context.get('schema_version')) is not int or context['schema_version'] != 1
                or not isinstance(context.get('nonce'), str) or not NONCE.fullmatch(context['nonce'])
                or context.get('product_completion') != 'incomplete'
                or context.get('next_stage_allowed') is not False
                or not isinstance(context.get('infrastructure_errors'), list)):
            raise ValueError('Actual binding invocation identity differs')
        result['infrastructure_errors'] += context['infrastructure_errors']
        begin, end = interval(context['started_at'], context['finished_at'])
        before = source_map(load(within(root, context['source_before'], directory)))
        after = source_map(load(within(root, context['source_after'], directory)))
        current_policy = load(path_at(root, 'openspec/changes/align-approved-binding-journey/phase-policy.json'))
        if current_policy.get('resume_epoch') is not None:
            claim_ref = context.get('nonce_claim')
            expected_claim_path = '.build/binding-journey-alignment-20261109/phase-2-r3/nonce-ledger/' + context['nonce'] + '.json'
            if not isinstance(claim_ref, dict) or claim_ref.get('path') != expected_claim_path:
                raise ValueError('Actual fresh exclusive resumption nonce allocation required')
            claim = load(checked_ref(root, claim_ref, ignored=True))
            if (claim.get('kind') != 'binding-resumption-nonce-claim/v1' or claim.get('schema_version') != 1
                    or claim.get('nonce') != context['nonce'] or claim.get('output') != directory.relative_to(root).as_posix()
                    or claim.get('source_tree_sha256') != tree_sha(before)
                    or type(context.get('pid')) is not int or context['pid'] < 1 or claim.get('pid') != context['pid']
                    or datetime.fromisoformat(claim['claimed_at']) > begin):
                raise ValueError('Resumption nonce claim belongs to another source/process/invocation')
        result['source_tree_sha256'] = tree_sha(before)
        if (after != before or context.get('source_tree_sha256') != tree_sha(before)
                or expected_source_tree is not None and expected_source_tree != tree_sha(before)
                or require_current and source_files(root) != before):
            raise ValueError('Execution source epoch changed or differs from expected source')
        rust_replay = 'required_current_tree'
        if current_policy.get('resume_epoch') is not None and not require_current:
            current_source = source_files(root)
            if current_source != before:
                if expected_source_tree != tree_sha(before):
                    raise ValueError('Historical replay disposition requires the actual independently reviewed P2 source identity')
                from binding_phase_guard import verify_r3_historical_admission
                result['historical_admission'] = verify_r3_historical_admission(root, current_policy, before, current_source)
                rust_replay = 'not_run_historical_workspace_dependent'
        commands = context.get('executions')
        if not isinstance(commands, list) or len(commands) != 2:
            raise ValueError('Both actual production exporter and component commands required')
        previous = begin
        for role, row in zip(('production-projections', 'real-components'), commands):
            if (row.get('role') != role or row.get('cwd') != '.' or type(row.get('exit_code')) is not int):
                raise ValueError('Actual command role/cwd/exit differs')
            a, b = interval(row['started_at'], row['finished_at'])
            if not previous <= a <= b <= end:
                raise ValueError('Commands are not in the actual invocation interval')
            previous = b
            for key in ('stdout', 'stderr'):
                log = checked_ref(root, row[key], ignored=True)
                if not log.is_relative_to(directory):
                    raise ValueError('Command logs belong to another execution')
        export, component = commands
        relative = directory.relative_to(root).as_posix()
        expected_export = [sys.executable, EXPORTER, '--output', relative + '/projections', '--nonce', context['nonce']]
        if context.get('runtime_transport_producer') is not None:
            transport_path = checked_ref(root, context['runtime_transport_producer'], ignored=True)
            inspect_transport(root, load(transport_path), context['nonce'], before)
            expected_export += ['--runtime-transport', context['runtime_transport_producer']['path']]
        if export['command'] != expected_export or export['exit_code'] != 0:
            raise ValueError('Actual production exporter command/exit differs')
        call = component['command']
        if (len(call) != 8 or Path(call[0]).name != 'node' or call[1] != RUNNER
                or call[2:] != ['--output', relative + '/component', '--nonce', context['nonce'],
                                '--projection-fixtures', relative + '/projections/manifest.json']
                or component['exit_code'] not in (0, 1, 3)):
            raise ValueError('Actual component command/exit differs; infrastructure is not semantic RED')
        bundle = inspect_component(root, context['component_report'], context['nonce'], before)
        rust = inspect_rust_layers(root, context['rust_layers'], context['nonce'], before)
        result['layer_results'] = rust['results']
        result['storage_v5_inspection'] = rust['storage_v5_inspection']
        from binding_phase_guard import inspect_guard_run
        guard = inspect_guard_run(root, context['guard_execution'])
        if guard['source_tree_before_sha256'] != tree_sha(before):
            raise ValueError('Binding guard tests belong to a different source epoch')
        result['layer_results'].append({'case_id': 'BJ-18', 'layer': 'guard', 'status': 'passed',
                                       'execution': context['guard_execution']})
        if context.get('native_evidence') is not None:
            result['layer_results'] += inspect_native_collection(root, context['native_evidence'], context['nonce'], before)
        if context.get('service_evidence') is not None:
            result['layer_results'].append(inspect_live_service(root, context['service_evidence'], context['nonce'], before))
        result['execution_obligations'] = inspect_execution_obligations(root, context, before, bundle, rust, result['layer_results'])
        result['infrastructure_errors'] += rust['infrastructure_errors']
        bundle['rust_layers'] = context['rust_layers']
        bundle['source_map'] = before
        bundle['rust_replay_disposition'] = rust_replay
        report = bundle['report']
        reported_command = report['command']
        if (Path(reported_command[0]).resolve() != Path(call[0]).resolve()
                or Path(reported_command[1]).resolve() != root / RUNNER
                or reported_command[2:] != call[2:]):
            raise ValueError('Component report command differs from actual invocation')
        a, b = interval(report['started_at'], report['finished_at'])
        if not datetime.fromisoformat(component['started_at']) <= a <= b <= datetime.fromisoformat(component['finished_at']):
            raise ValueError('Component report outside its real command interval')
        judgments = bundle['judgments'] + bundle['scenes']['judgments']
        result.update(case_count=len(judgments), passed=sum(x['status'] == 'passed' for x in judgments),
                      failed=sum(x['status'] == 'failed' for x in judgments),
                      semantic_failures=[{'case_id': x['id'], 'assertion_ids': x['failed_assertions']}
                                         for x in judgments if x['status'] == 'failed'])
        blocked_count = sum(x['status'] == 'blocked' for x in judgments)
        result['blocked'] = blocked_count
        result['diagnostic_count'] = len(bundle['judgments'])
        result['scene_count'] = len(bundle['scenes']['judgments'])
        result['layer_results'] += bundle['scenes']['layer_results']
        expected_code = 1 if result['failed'] else 3 if blocked_count else 0
        if component['exit_code'] != expected_code:
            raise ValueError('Actual component exit contradicts independently evaluated behavior')
        from binding_journey_recompute import recompute
        independent = recompute(root, bundle)
        result['recomputation'] = independent
        if independent['errors']:
            raise ValueError('Independent retained-DOM/producer recomputation failed: ' + '; '.join(independent['errors']))
        result['eligible_semantic_red'] = bool(result['failed']) and not result['infrastructure_errors']
        layer_failures = [row for row in result['layer_results'] if row['status'] == 'failed']
        result['layer_failures'] = layer_failures
        obligation_failures = [row for row in result['execution_obligations']['groups'] if row['status'] == 'failed']
        obligation_blocked = [row for row in result['execution_obligations']['groups'] if row['status'] in {'blocked','not_run'}]
        result['obligation_failures'] = obligation_failures
        result['status'] = ('failed' if result['failed'] or layer_failures or obligation_failures or result['infrastructure_errors']
                            else 'blocked' if blocked_count or obligation_blocked else 'passed')
        result['execution'] = ref(root, relative + '/execution.json')
        result['component_report'] = context['component_report']
        result['assertion_count'] = sum(len(x['assertions']) for x in judgments)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError) as error:
        result['validation_errors'].append(str(error))
    result['errors'] = result['validation_errors'] + result['infrastructure_errors'] + [
        x['case_id'] + ': ' + ', '.join(x['assertion_ids']) for x in result['semantic_failures']]
    result['errors'] += [row['case_id'] + '/' + row['layer'] + ': failed' for row in result.get('layer_failures', [])]
    result['errors'] += [row['id'] + ': failed' for row in result.get('obligation_failures', [])]
    if result['status'] == 'blocked':
        result['errors'].append('Actual component scene input/capability remains blocked')
    if result['validation_errors'] or result['infrastructure_errors']:
        result.update(status='failed', eligible_semantic_red=False)
    return result
