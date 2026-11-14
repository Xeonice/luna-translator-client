import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import ts from 'typescript';

// Exercise the production TypeScript function, rather than a parallel test implementation.
const sourceText = await fs.readFile(new URL('../../src/features/model-config/service-management.ts', import.meta.url), 'utf8');
const compiled = ts.transpileModule(sourceText, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, isolatedModules: true }, reportDiagnostics: true });
assert.deepEqual(compiled.diagnostics, []);
const { projectServiceManagement, serviceManagementState } = await import(`data:text/javascript;base64,${Buffer.from(compiled.outputText).toString('base64')}`);
const purposes = ['translation', 'grammar', 'context_word'];
const http = (revisionId = 'current', modelId = 'model-a') => ({ kind: 'http', source: { revisionId, modelId } });
const offline = (revisionId = 'current', configId = 'model-a') => ({ kind: 'offline', source: { revisionId, configId } });
const model = (modelId = 'model-a', extra = {}) => ({ modelId, connectionId: 'shared-connection', displayName: '同名服务', defaultTemplates: {}, actualModelId: 'same-actual-model', selectionSource: 'manual', capabilities: {}, parameters: {}, ...extra });
const binding = (purpose, modelId = 'model-a', kind = 'legacy') => ({ purpose, bindingId: purpose, scopeKey: 'global', bindingMode: 'override', enabled: true, engines: [kind === 'legacy' ? { modelId, engineId: `${purpose}-engine`, enabled: true } : { target: { kind: 'http', modelId }, engineId: `${purpose}-engine`, enabled: true }] });
function configuration({ revisionId = 'current', models = [model()], bindings = [], templates = [], kind = 'legacy', offlineEngineConfigs = [], applied = null } = {}) {
  const graph = { revisionId, models, bindings, templates, connections: [{ connectionId: 'shared-connection' }], termSets: [], ...(kind === 'service_graph' ? { offlineEngineConfigs, codecVersion: 3 } : {}) };
  return { profileGeneration: 1, draftHeadRevision: 11, appliedHeadRevision: 5, draft: { kind, configuration: graph }, applied, runtimePurposes: [], runtimeConnections: [] };
}
function row(source = http(), purpose = 'translation', extra = {}) {
  return { source, purpose, logicalResourceId: `${source.kind}:${source.kind === 'http' ? source.source.modelId : source.source.configId}`, displayName: '同名服务', backend: source.kind === 'http' ? 'http_llm' : 'local_worker', eligibility: { kind: 'selectable' }, verification: { kind: 'untested' }, ...extra };
}
function listings(rows, selected = {}) {
  return purposes.map(purpose => ({ purpose, projection: { profileGeneration: 1, appliedHeadRevision: 5, rows: rows.filter(value => value.purpose === purpose), selection: { requestedScope: { kind: 'global' }, effectiveScope: { kind: 'global' }, selectionRevision: 2, effectiveSelectionRevision: 2, purposeEnabled: false, main: selected[purpose]?.[0] ?? null, comparisons: selected[purpose]?.slice(1) ?? [] } } }));
}
function freeze(value) { if (value && typeof value === 'object') { Object.freeze(value); Object.values(value).forEach(freeze); } return value; }
const facts = group => group.sources.flatMap(source => source.purposes.map(value => value.purpose));
const mismatch = /服务资源与当前配置版本不一致/;

test('ten immutable versions times three queries produce one current legacy resource and keep all raw identities', () => {
  const revisions = ['z-last-looking', 'old-2', 'old-3', 'old-4', 'old-5', 'old-6', 'old-7', 'old-8', 'old-9', 'current'];
  const raw = revisions.flatMap(revision => purposes.map(purpose => row(http(revision), purpose, { eligibility: { kind: 'blocked', reasons: [{ code: 'legacy_requires_guided_configuration', message: '已有配置可继续运行；新选用需要引导配置。', fieldId: null }] } })));
  const config = configuration({ bindings: purposes.map(purpose => binding(purpose)) });
  const input = listings(raw, Object.fromEntries(purposes.map(purpose => [purpose, [http()]])));
  const before = structuredClone({ config, input }); freeze(config); freeze(input);
  const groups = projectServiceManagement(config, input);
  assert.equal(raw.length, 30); assert.equal(groups.length, 1); assert.equal(groups[0].sources.length, 1);
  assert.deepEqual(groups[0].editor, { source: http(), purpose: 'translation' });
  assert.deepEqual(facts(groups[0]), purposes);
  for (const fact of groups[0].sources[0].purposes) {
    assert.equal(fact.current, true); assert.equal(fact.bound, true); assert.equal(fact.declared, false);
    assert.equal(fact.row, raw.find(value => value.purpose === fact.purpose && value.source.source.revisionId === 'current'));
    assert.match(serviceManagementState(fact.row), /旧版配置.*尚未测试/);
  }
  assert.deepEqual({ config, input }, before);
  assert.equal(input.reduce((count, value) => count + value.projection.rows.length, 0), 30);
});

test('same names, actual model names and shared connections do not merge distinct model IDs', () => {
  const config = configuration({ models: [model('a'), model('b')], bindings: [binding('translation', 'a'), binding('translation', 'b')] });
  const groups = projectServiceManagement(config, listings([row(http('current', 'b')), row(http('current', 'a'))]));
  assert.equal(groups.length, 2);
  assert.deepEqual(groups.map(group => group.editor.source.source.modelId), ['a', 'b']);
  assert.equal(new Set(groups.map(group => group.logicalResourceId)).size, 2);
});

test('offline and HTTP resources with the same literal ID and name remain separate', () => {
  const config = configuration({ kind: 'service_graph', models: [model('model-a', { templatePurpose: 'translation' })], offlineEngineConfigs: [{ id: 'model-a', displayName: '同名服务' }] });
  const groups = projectServiceManagement(config, listings([row(http()), row(offline())]));
  assert.equal(groups.length, 2); assert.deepEqual(groups.map(group => group.editor.source.kind), ['http', 'offline']);
  assert.deepEqual(facts(groups[1]), ['translation']);
});

test('new draft and explicitly selected old sources share a row, edit the draft, and retain separate verification objects', () => {
  const old = http('old-active'), current = http(), failure = { kind: 'failed', testId: 'old-failure', code: 'http_403', testedAtMs: 3n };
  const passed = { kind: 'passed', testId: 'new-pass', testedAtMs: 4n };
  const oldRow = row(old, 'translation', { verification: failure }), draftRow = row(current, 'translation', { verification: passed });
  const config = configuration({ bindings: [binding('translation')], applied: { kind: 'legacy', configuration: { revisionId: 'activation-materialized', models: [model('unresolved-alias')], bindings: [], templates: [], connections: [], termSets: [] } } });
  const [group] = projectServiceManagement(config, listings([oldRow, draftRow], { translation: [old] }));
  assert.equal(group.sources.length, 2); assert.equal(group.editor.source, current);
  assert.equal(group.sources[0].draft, true); assert.equal(group.sources[0].purposes[0].current, false);
  assert.equal(group.sources[1].draft, false); assert.equal(group.sources[1].purposes[0].current, true);
  assert.equal(group.sources[0].purposes[0].row.verification, passed);
  assert.equal(group.sources[1].purposes[0].row.verification, failure);
  assert.match(serviceManagementState(oldRow), /测试失败（http_403）/);
});

test('only explicit selected applied-only sources are added; dormant applied and history-only resources remain raw', () => {
  const active = http('old', 'active-only'), dormant = http('old', 'dormant'), comparison = http('old', 'comparison');
  const applied = { kind: 'legacy', configuration: { revisionId: 'old', models: [model('active-only'), model('dormant'), model('comparison')], bindings: [], templates: [], connections: [], termSets: [] } };
  const config = configuration({ models: [], applied });
  const raw = [row(dormant), row(active), row(comparison)];
  const groups = projectServiceManagement(config, listings(raw, { translation: [active, comparison] }));
  assert.deepEqual(groups.map(group => group.logicalResourceId), ['http:active-only', 'http:comparison']);
  assert.ok(groups.every(group => group.sources[0].draft === false));
  assert.equal(groups[0].editor.source, active); assert.equal(groups[1].editor.source, comparison);
  assert.equal(raw.length, 3);
});

test('declared v3 purpose is accurate and legacy query presence never invents three purposes', () => {
  const grammar = configuration({ kind: 'service_graph', models: [model('model-a', { templatePurpose: 'grammar' })] });
  assert.deepEqual(facts(projectServiceManagement(grammar, listings([row(http(), 'grammar')]))[0]), ['grammar']);
  const legacy = configuration(), raw = purposes.map(purpose => row(http(), purpose));
  const [unknown] = projectServiceManagement(legacy, listings(raw));
  assert.deepEqual(facts(unknown), []); assert.equal(unknown.editor, null); assert.equal(unknown.sources[0].rows.length, 3);
});

test('explicit v3 purpose takes precedence over retained valid templates for other purposes', () => {
  const config = configuration({
    kind: 'service_graph',
    models: [model('model-a', { templatePurpose: 'grammar', defaultTemplates: { translation: 'retained-translation', grammar: 'current-grammar', context_word: 'retained-context' } })],
    templates: [{ templateId: 'retained-translation', purpose: 'translation' }, { templateId: 'current-grammar', purpose: 'grammar' }, { templateId: 'retained-context', purpose: 'context_word' }],
    bindings: [binding('grammar', 'model-a', 'service_graph')],
  });
  const grammarRow = row(http(), 'grammar'), input = listings([grammarRow]);
  const before = structuredClone({ config, input }); freeze(config); freeze(input);
  const [group] = projectServiceManagement(config, input);
  assert.deepEqual(facts(group), ['grammar']);
  assert.deepEqual(group.editor, { source: http(), purpose: 'grammar' });
  assert.equal(group.sources[0].purposes[0].declared, true);
  assert.equal(group.sources[0].purposes[0].row, grammarRow);
  assert.deepEqual({ config, input }, before);
});

test('real referenced templates and configured bindings supply purpose facts, including disabled bindings', () => {
  const config = configuration({ models: [model('model-a', { defaultTemplates: { grammar: 'real-grammar-template', context_word: 'missing-template' } })], templates: [{ templateId: 'real-grammar-template', purpose: 'grammar' }], bindings: [{ ...binding('translation'), enabled: false }] });
  const [group] = projectServiceManagement(config, listings(purposes.map(purpose => row(http(), purpose))));
  assert.deepEqual(facts(group), ['translation', 'grammar']);
  assert.equal(group.sources[0].purposes[0].bound, true);
  assert.equal(group.sources[0].purposes[1].declared, true);
});

test('unknown-purpose current draft does not silently redirect editing to an active historical version', () => {
  const old = http('old');
  const [group] = projectServiceManagement(configuration(), listings([row(http()), row(old)], { translation: [old] }));
  assert.equal(group.editor, null); assert.equal(group.sources.length, 2);
  assert.deepEqual(group.sources[1].purposes.map(value => value.purpose), ['translation']);
});

test('missing draft or selected exact refs cannot be replaced by same-name, highest-looking version, or materialized alias', () => {
  const old = row(http('zzzz-highest'));
  assert.throws(() => projectServiceManagement(configuration(), listings([old])), mismatch);
  assert.throws(() => projectServiceManagement(configuration({ models: [] }), listings([old], { translation: [http('activation-materialized', 'alias')] })), mismatch);
  assert.throws(() => projectServiceManagement(configuration({ bindings: [binding('grammar')] }), listings([row(http())])), mismatch);
});

test('mismatched profile/head/purpose cohorts are rejected without mutating inputs', () => {
  const config = configuration({ bindings: [binding('translation')] });
  for (const mutate of [input => input[1].projection.profileGeneration++, input => input[1].projection.appliedHeadRevision++, input => { input[1].purpose = 'translation'; }, input => { input[0].projection.rows[0].purpose = 'grammar'; }, input => { input.pop(); }]) {
    const input = listings([row(http())]); mutate(input); const before = structuredClone(input); freeze(input);
    assert.throws(() => projectServiceManagement(config, input), mismatch); assert.deepEqual(input, before);
  }
});

test('per-purpose partially tested, stale, failed, passed and blocked states are preserved exactly', () => {
  const states = [{ kind: 'partially_tested', testId: 'pool', passedCredentialCount: 1, totalCredentialCount: 3, testedAtMs: 2n }, { kind: 'stale', testId: 'stale' }, { kind: 'failed', testId: 'failure', code: 'revoked', testedAtMs: 2n }, { kind: 'passed', testId: 'pass', testedAtMs: 2n }];
  const expected = [/已测凭据 1\/3/, /测试已过期/, /测试失败（revoked）/, /本项测试通过/];
  states.forEach((verification, index) => {
    const current = row(http(), 'translation', { verification, eligibility: { kind: 'blocked', reasons: [{ code: 'credential_revoked', message: '凭据已撤销。', fieldId: 'api_key' }] } });
    const [group] = projectServiceManagement(configuration({ bindings: [binding('translation')] }), listings([current]));
    const actual = group.sources[0].purposes[0].row;
    assert.equal(actual, current); assert.equal(actual.verification, verification); assert.equal(actual.eligibility, current.eligibility);
    assert.match(serviceManagementState(actual), expected[index]); assert.match(serviceManagementState(actual), /暂不可选用/);
  });
});
