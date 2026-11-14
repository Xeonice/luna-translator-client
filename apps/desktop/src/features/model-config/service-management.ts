import type { CatalogPurpose, SavedServiceProjection, SavedServiceRef, SavedServiceRow, ServiceConfigurationSnapshot } from '../../bridge/contracts.generated';

export type ServiceManagementListing = { purpose: CatalogPurpose; projection: SavedServiceProjection };
export type ServiceManagementPurpose = { purpose: CatalogPurpose; declared: boolean; bound: boolean; current: boolean; row: SavedServiceRow };
export type ServiceManagementSource = { source: SavedServiceRef; draft: boolean; rows: SavedServiceRow[]; purposes: ServiceManagementPurpose[] };
export type ServiceManagementGroup = { key: string; logicalResourceId: string; displayName: string; backend: SavedServiceRow['backend']; sources: ServiceManagementSource[]; editor: { source: SavedServiceRef; purpose: CatalogPurpose } | null };

const purposes: CatalogPurpose[] = ['translation', 'grammar', 'context_word'];
const isPurpose = (value: string): value is CatalogPurpose => purposes.includes(value as CatalogPurpose);
export const serviceManagementSourceKey = (source: SavedServiceRef) => JSON.stringify(source.kind === 'http' ? ['http', source.source.revisionId, source.source.modelId] : ['offline', source.source.revisionId, source.source.configId]);
const inconsistent = () => new Error('服务资源与当前配置版本不一致，未替换列表或当前输入。请重新读取。');

/** A settings view over Rust's exact references, never a replacement for its saved-version catalogue. */
export function projectServiceManagement(configuration: ServiceConfigurationSnapshot, listings: readonly ServiceManagementListing[]): ServiceManagementGroup[] {
  if (listings.length !== purposes.length || purposes.some(purpose => listings.filter(list => list.purpose === purpose).length !== 1) || listings.some(({ projection }) => projection.profileGeneration !== configuration.profileGeneration || projection.appliedHeadRevision !== configuration.appliedHeadRevision)) throw inconsistent();
  const rowsBySource = new Map<string, SavedServiceRow[]>();
  for (const { purpose, projection } of listings) for (const row of projection.rows) {
    if (row.purpose !== purpose) throw inconsistent();
    const key = serviceManagementSourceKey(row.source), existing = rowsBySource.get(key) ?? [];
    if (existing.some(value => value.purpose === purpose || value.logicalResourceId !== row.logicalResourceId || value.backend !== row.backend)) throw inconsistent();
    rowsBySource.set(key, [...existing, row]);
  }
  const sources = new Map<string, ServiceManagementSource>();
  const addSource = (source: SavedServiceRef, draft: boolean): ServiceManagementSource => {
    const key = serviceManagementSourceKey(source), existing = sources.get(key);
    if (existing) { existing.draft ||= draft; return existing; }
    const rows = rowsBySource.get(key);
    if (!rows?.length) throw inconsistent();
    const value = { source: rows[0].source, draft, rows, purposes: [] };
    sources.set(key, value); return value;
  };
  const addPurpose = (source: ServiceManagementSource, purpose: CatalogPurpose, fact: 'declared' | 'bound' | 'current') => {
    let value = source.purposes.find(item => item.purpose === purpose);
    if (!value) {
      const row = source.rows.find(item => item.purpose === purpose);
      if (!row) throw inconsistent();
      value = { purpose, declared: false, bound: false, current: false, row };
      source.purposes.push(value);
    }
    value[fact] = true;
  };
  const document = configuration.draft, graph = document.configuration;
  for (const model of graph.models) {
    const source = addSource({ kind: 'http', source: { revisionId: graph.revisionId, modelId: model.modelId } }, true);
    if ('templatePurpose' in model && model.templatePurpose) {
      // Rust restricts this resource's saved rows to its explicit purpose; retained
      // defaults for other purposes are not additional service declarations.
      addPurpose(source, model.templatePurpose, 'declared');
    } else {
      for (const [purpose, templateId] of Object.entries(model.defaultTemplates)) if (isPurpose(purpose) && graph.templates.some(template => template.templateId === templateId && template.purpose === purpose)) addPurpose(source, purpose, 'declared');
      for (const binding of graph.bindings) if (isPurpose(binding.purpose) && binding.engines.some(engine => 'modelId' in engine ? engine.modelId === model.modelId : engine.target.kind === 'http' && engine.target.modelId === model.modelId)) addPurpose(source, binding.purpose, 'bound');
    }
  }
  if (document.kind === 'service_graph') for (const offline of document.configuration.offlineEngineConfigs) {
    const source = addSource({ kind: 'offline', source: { revisionId: graph.revisionId, configId: offline.id } }, true);
    addPurpose(source, 'translation', 'declared');
    if (document.configuration.bindings.some(binding => binding.purpose === 'translation' && binding.engines.some(engine => engine.target.kind === 'offline' && engine.target.offlineConfigId === offline.id))) addPurpose(source, 'translation', 'bound');
  }
  // Selection refs have already been resolved by Rust, including activation aliases.
  for (const { purpose, projection } of listings) for (const source of [projection.selection.main, ...projection.selection.comparisons]) if (source) addPurpose(addSource(source, false), purpose, 'current');
  const groups = new Map<string, ServiceManagementGroup>();
  for (const source of sources.values()) {
    source.purposes.sort((a, b) => purposes.indexOf(a.purpose) - purposes.indexOf(b.purpose));
    const row = source.rows[0], key = JSON.stringify([source.source.kind, row.logicalResourceId]);
    let group = groups.get(key);
    if (!group) { group = { key, logicalResourceId: row.logicalResourceId, displayName: row.displayName, backend: row.backend, sources: [], editor: null }; groups.set(key, group); }
    if (source.draft && group.sources.some(value => value.draft)) throw inconsistent();
    group.sources.push(source);
    if (source.draft || (!group.sources.some(value => value.draft) && !group.editor)) {
      if (source.draft) { group.displayName = row.displayName; group.backend = row.backend; }
      const purpose = source.purposes[0]?.purpose;
      group.editor = purpose ? { source: source.source, purpose } : null;
    }
  }
  return [...groups.values()];
}

/** Keep each purpose's Rust status separate, including blocked reasons and prior failures. */
export function serviceManagementState(row: SavedServiceRow): string {
  const verification = row.verification;
  const tested = verification.kind === 'passed' ? '本项测试通过' : verification.kind === 'partially_tested' ? `已测凭据 ${verification.passedCredentialCount}/${verification.totalCredentialCount}` : verification.kind === 'failed' ? `测试失败（${verification.code}）` : verification.kind === 'stale' ? '测试已过期' : '尚未测试';
  if (row.eligibility.kind !== 'blocked') return tested;
  return `${row.eligibility.reasons.some(reason => reason.code === 'legacy_requires_guided_configuration') ? '旧版配置' : '暂不可选用'} · ${tested}`;
}
