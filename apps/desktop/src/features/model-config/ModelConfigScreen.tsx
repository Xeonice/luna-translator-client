import { useCallback, useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import type { AppSnapshot, CatalogPurpose, CatalogScalar, Configuration, SavedServiceRef, SavedServiceRow, ServiceAuthenticationSelection, ServiceCatalog, ServiceConfigurationSnapshot, ServiceCredentialInput, ServiceFormTarget, ServiceScope, GoogleInteractiveContext, GoogleInteractiveProxy } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { useAction } from '../../bridge/projections';
import { Button, Select, Sheet } from '../../ui/primitives/Controls';
import { Notice } from '../../ui/patterns/Patterns';
import { ServiceCatalogPicker, guidedPurposeNames } from './ServiceCatalogPicker';
import type { ServiceCatalogSelection } from './ServiceCatalogPicker';
import { GuidedServiceEditor, useGuidedServiceForm } from './GuidedServiceEditor';
import type { GuidedServiceFieldAction } from './GuidedServiceEditor';
import { ServiceSelector, savedServiceKey } from './ServiceSelector';
import { CredentialRevocationPanel } from './CredentialRevocationPanel';
import { TermSetEditor } from './TermSetEditor';
import { CredentialPoolEditor } from './CredentialPoolEditor';
import type { PoolDraftCache } from './CredentialPoolEditor';
import { useGoogleAuthorization } from './useGoogleAuthorization';
import { RuntimeConfigurationPanel } from './RuntimeConfigurationPanel';
import { projectServiceManagement, serviceManagementSourceKey, serviceManagementState } from './service-management';
import type { ServiceManagementGroup } from './service-management';
import styles from './styles.module.css';

type RouteIntent = { section: 'connection'; sequence: number };
type Editor = ServiceCatalogSelection & { key: string; source?: SavedServiceRef; values: Record<string, CatalogScalar>; authentication: ServiceAuthenticationSelection; expectedHead: number; readonlyReason: string | null; shared: string[]; fallback: SavedServiceRef[] };
type Next = { kind: 'catalog'; purpose: CatalogPurpose } | { kind: 'edit'; source: SavedServiceRef; purpose: CatalogPurpose } | { kind: 'cancel' };
const purposes: CatalogPurpose[] = ['translation', 'grammar', 'context_word'];
const backendNames = { http_llm: '大模型服务', http_translation: '云端翻译', platform_native: '系统翻译', local_worker: '本机离线翻译' };

function serviceLabel(target: ServiceFormTarget, catalog: ServiceCatalog | null) {
  if (target.kind === 'http') { const preset = target.preset; return catalog?.presets.find(item => item.key.presetId === preset.presetId && item.key.presetRevision === preset.presetRevision)?.label ?? '已有服务'; }
  return catalog?.offlineEngines.find(item => item.engineId === target.engineId)?.label ?? '本机服务';
}
/** Preserve the exact saved connection's existing policy; no new network form is invented. */
function googleProxy(editor: Editor, configuration: ServiceConfigurationSnapshot | null): GoogleInteractiveProxy | null {
  if (!editor.source) return { mode: 'inherit' };
  if (editor.source.kind !== 'http' || !configuration) return null;
  const source = editor.source.source;
  const document = [configuration.draft, configuration.applied].find(value => value?.configuration.revisionId === source.revisionId);
  if (!document) return null;
  const model = document.configuration.models.find(value => value.modelId === source.modelId);
  const connection = document.configuration.connections.find(value => value.connectionId === model?.connectionId);
  if (!connection || !['inherit', 'disabled', 'override'].includes(connection.proxyMode)) return null;
  if (connection.proxyMode === 'override') return { mode: 'override', proxyUrl: typeof connection.networkMetadata.proxyUrl === 'string' ? connection.networkMetadata.proxyUrl : '' };
  return { mode: connection.proxyMode === 'disabled' ? 'disabled' : 'inherit' };
}
function LegacyMaintenance({ configuration, snapshot, store, onDirty }: { configuration: Configuration; snapshot: AppSnapshot; store: ApplicationStore; onDirty: (value: boolean) => void }) {
  const [draft, setDraft] = useState(() => structuredClone(configuration)), [dirty, setDirty] = useState(false), [termId, setTermId] = useState(''), [connectionId, setConnectionId] = useState('');
  const head = useRef(snapshot.configuration.draftHeadRevision), cache = useRef<PoolDraftCache>(new Map()), action = useAction();
  useEffect(() => { if (!dirty) { setDraft(structuredClone(configuration)); head.current = snapshot.configuration.draftHeadRevision; } }, [configuration, snapshot.configuration.draftHeadRevision, dirty]);
  useEffect(() => { onDirty(dirty); return () => onDirty(false); }, [dirty, onDirty]);
  const update = (value: Configuration) => { setDraft(value); setDirty(true); };
  const term = draft.termSets.find(item => item.termSetId === termId), connection = draft.connections.find(item => item.connectionId === connectionId);
  return <details className={styles.disclosure} data-configuration-section="legacy-maintenance"><summary>已有资料与共用账号<small>术语表、凭据池及已应用运行控制</small></summary><div className={styles.disclosureBody}>
    <p className={styles.help}>维护原有配置文档；不会把它转换为本机离线服务，也不会在保存时切换当前服务。</p>
    <Select label="选择术语表" value={termId} onChange={setTermId} options={[{ value: '', label: '选择术语表…' }, ...draft.termSets.map(item => ({ value: item.termSetId, label: item.displayName }))]}/>
    <Button onPress={() => { const id = crypto.randomUUID(); update({ ...draft, termSets: [...draft.termSets, { termSetId: id, scopeKey: 'global', gameId: null, displayName: '新术语表', enabled: true, terms: [] }] }); setTermId(id); }}>添加术语表</Button>
    {term ? <TermSetEditor termSet={term} games={snapshot.games} onChange={value => update({ ...draft, termSets: draft.termSets.map(item => item.termSetId === value.termSetId ? value : item) })}/> : null}
    <details className={styles.disclosure}><summary>多套凭据与共享连接</summary><div className={styles.disclosureBody}><Select label="选择已有连接" value={connectionId} onChange={setConnectionId} options={[{ value: '', label: '选择已有连接…' }, ...draft.connections.map(item => ({ value: item.connectionId, label: item.displayName }))]}/>{connection ? <CredentialPoolEditor snapshot={snapshot} connection={connection} store={store} draftCache={cache.current} onConnection={value => update({ ...draft, connections: draft.connections.map(item => item.connectionId === value.connectionId ? value : item) })}/> : null}</div></details>
    {action.error ? <Notice error>{action.error} 本地资料编辑已保留。</Notice> : null}
    <Button pending={action.pending} disabled={!dirty} onPress={() => void action.run(async () => { await store.command({ kind: 'save_configuration', payload: { configuration: draft, expectedHeadRevision: head.current } }); setDirty(false); })}>保存已有资料草稿</Button>
    <RuntimeConfigurationPanel snapshot={snapshot} store={store} selection={{ kind: 'purpose', scopeKey: 'global', purpose: 'translation' }}/>
  </div></details>;
}

/** Settings page. Rust owns graph hydration, required fields, authentication and selection. */
export function ModelConfigScreen({ snapshot, store, routeIntent, header, active = true, resetKey = 0, onDirtyChange }: { snapshot: AppSnapshot; store: ApplicationStore; routeIntent?: RouteIntent | null; header?: ReactNode; active?: boolean; resetKey?: number; onDirtyChange?: (dirty: boolean) => void }) {
  const [catalog, setCatalog] = useState<ServiceCatalog | null>(null), [configuration, setConfiguration] = useState<ServiceConfigurationSnapshot | null>(null), [rawRows, setRawRows] = useState<SavedServiceRow[]>([]), [managementGroups, setManagementGroups] = useState<ServiceManagementGroup[]>([]);
  const [editor, setEditor] = useState<Editor | null>(null), [unmapped, setUnmapped] = useState<string | null>(null), [dirty, setDirty] = useState(false), [legacyDirty, setLegacyDirty] = useState(false), [catalogPurpose, setCatalogPurpose] = useState<CatalogPurpose>('translation'), [catalogOpen, setCatalogOpen] = useState(false), [next, setNext] = useState<Next | null>(null);
  const [googleRevokeOpen, setGoogleRevokeOpen] = useState(false);
  const [loading, setLoading] = useState(true), [editorLoading, setEditorLoading] = useState(false), [saving, setSaving] = useState(false), [testing, setTesting] = useState(false), [cancellingTest, setCancellingTest] = useState(false), [error, setError] = useState<string | null>(null), [notice, setNotice] = useState<string | null>(null), [testResult, setTestResult] = useState<ReactNode>(null), [refreshKey, setRefreshKey] = useState(0), [scopeKey, setScopeKey] = useState('global');
  const runningTest = useRef<{ operationId: string; profileGeneration: number; label: string; cancelRequested: boolean } | null>(null);
  const epoch = useRef(0), editorEpoch = useRef(0), editRevision = useRef(0), testEpoch = useRef(0), startedEditor = useRef(false), lastViewed = useRef<{ source: SavedServiceRef; purpose: CatalogPurpose } | null>(null), lastProfile = useRef(snapshot.profileGeneration), secrets = useRef(new Map<string, string>()), secretRevision = useRef(0), storedSecret = useRef<{ revision: number; authentication: ServiceAuthenticationSelection } | null>(null), fileInput = useRef<HTMLInputElement>(null), listHeading = useRef<HTMLHeadingElement>(null);
  const profile = useRef(snapshot.profileGeneration), activeRef = useRef(active); profile.current = snapshot.profileGeneration; activeRef.current = active;
  const project = useCallback((request: Parameters<typeof store.port.serviceForm>[0]) => store.port.serviceForm(request), [store]);
  const form = useGuidedServiceForm(editor ? { target: editor.target, purpose: editor.purpose, values: editor.values } : null, project);
  const cancelTest = useCallback(async () => {
    const operation = runningTest.current;
    if (!operation || operation.cancelRequested) return;
    operation.cancelRequested = true; setCancellingTest(true);
    try { await store.port.cancelSavedServiceTest(operation.operationId, operation.profileGeneration); }
    catch (reason) { if (runningTest.current === operation) { operation.cancelRequested = false; setError(`未能取消“${operation.label}”的测试：${errorMessage(reason)}`); } }
    finally { if (runningTest.current === operation) setCancellingTest(false); }
  }, [store]);
  const refresh = useCallback(async () => {
    const generation = ++epoch.current; setLoading(true); setError(null);
    try {
      const [nextCatalog, nextConfiguration, ...lists] = await Promise.all([store.port.serviceCatalog(), store.port.serviceConfiguration(), ...purposes.map(purpose => store.port.savedServices({ expectedProfileGeneration: snapshot.profileGeneration, scope: { kind: 'global' }, purpose }))]);
      if (generation !== epoch.current || profile.current !== snapshot.profileGeneration) return;
      if (nextConfiguration.profileGeneration !== snapshot.profileGeneration || lists.some(list => list.profileGeneration !== snapshot.profileGeneration)) throw new Error('配置档已变化，请重新读取。');
      const groups = projectServiceManagement(nextConfiguration, lists.map((projection, index) => ({ purpose: purposes[index], projection })));
      setCatalog(nextCatalog); setConfiguration(nextConfiguration); setRawRows(lists.flatMap(list => list.rows)); setManagementGroups(groups);
    } catch (reason) { if (generation === epoch.current) setError(errorMessage(reason)); }
    finally { if (generation === epoch.current) setLoading(false); }
  }, [store, snapshot.profileGeneration]);
  useEffect(() => { void refresh(); return () => { epoch.current++; }; }, [refresh, snapshot.configuration.draftHeadRevision, snapshot.configuration.appliedHeadRevision]);
  useEffect(() => { onDirtyChange?.(dirty || legacyDirty); }, [dirty, legacyDirty, onDirtyChange]);
  useEffect(() => () => { epoch.current++; editorEpoch.current++; testEpoch.current++; secrets.current.clear(); }, []);
  useEffect(() => { if (!active) secrets.current.clear(); }, [active]);
  useEffect(() => { if (routeIntent) { listHeading.current?.focus({ preventScroll: true }); listHeading.current?.scrollIntoView({ block: 'start' }); } }, [routeIntent?.sequence]);
  const clearEdit = () => { void cancelTest(); editorEpoch.current++; testEpoch.current++; editRevision.current++; secrets.current.clear(); storedSecret.current = null; setUnmapped(null); setEditor(null); setDirty(false); setTestResult(null); };
  const openSaved = useCallback(async (source: SavedServiceRef, purpose: CatalogPurpose) => {
    void cancelTest();
    const generation = ++editorEpoch.current, requestedEditRevision = editRevision.current; startedEditor.current = true; lastViewed.current = { source, purpose }; secrets.current.clear(); storedSecret.current = null; setEditorLoading(true); setError(null); setTestResult(null);
    try {
      const value = await store.port.serviceDraft({ expectedProfileGeneration: snapshot.profileGeneration, source, purpose });
      if (generation !== editorEpoch.current || profile.current !== snapshot.profileGeneration) return;
      if (value.profileGeneration !== snapshot.profileGeneration || savedServiceKey(value.source) !== savedServiceKey(source) || value.purpose !== purpose) throw new Error('服务身份或配置档已变化，未替换当前编辑。');
      if (editRevision.current !== requestedEditRevision) { setError('读取期间当前输入已变化，未用迟到的配置覆盖编辑。'); return; }
      if (!value.target) { setEditor(null); setUnmapped(value.readonlyReason ?? '无法核对这项已有服务的精确接口，请查看已有资料。'); setDirty(false); return; }
      setUnmapped(null);
      setEditor({ key: crypto.randomUUID(), target: value.target, purpose: value.purpose, source: value.editing ?? undefined, values: value.values, authentication: value.authentication, expectedHead: value.draftHeadRevision, readonlyReason: value.readonlyReason, shared: value.affectedSharedModelIds, fallback: value.fallback }); setDirty(false); editRevision.current++; testEpoch.current++;
    } catch (reason) { if (generation === editorEpoch.current) setError(errorMessage(reason)); }
    finally { if (generation === editorEpoch.current) setEditorLoading(false); }
  }, [store, snapshot.profileGeneration, cancelTest]);
  useEffect(() => {
    const previous = lastProfile.current === snapshot.profileGeneration ? lastViewed.current : null;
    if (lastProfile.current !== snapshot.profileGeneration) setScopeKey('global');
    lastProfile.current = snapshot.profileGeneration;
    if (!previous) lastViewed.current = null;
    editorEpoch.current++; testEpoch.current++; editRevision.current++; secrets.current.clear(); setEditor(null); setDirty(false); setNotice(null); setTestResult(null); storedSecret.current = null; startedEditor.current = false;
    if (previous) void openSaved(previous.source, previous.purpose);
  }, [snapshot.profileGeneration, resetKey, openSaved]);
  useEffect(() => { const first = managementGroups.find(group => group.editor)?.editor; if (!loading && configuration?.profileGeneration === snapshot.profileGeneration && !startedEditor.current && first) { startedEditor.current = true; void openSaved(first.source, first.purpose); } }, [managementGroups, loading, openSaved, configuration?.profileGeneration, snapshot.profileGeneration]);
  const perform = (intent: Next) => { setNext(null); if (intent.kind === 'edit') void openSaved(intent.source, intent.purpose); else if (intent.kind === 'catalog') { setCatalogPurpose(intent.purpose); setCatalogOpen(true); } else clearEdit(); };
  const request = (intent: Next) => { if (dirty) setNext(intent); else perform(intent); };
  const add = (selection: ServiceCatalogSelection) => { clearEdit(); startedEditor.current = true; setEditor({ ...selection, key: crypto.randomUUID(), values: {}, authentication: { kind: 'none' }, expectedHead: configuration?.draftHeadRevision ?? snapshot.configuration.draftHeadRevision, readonlyReason: null, shared: [], fallback: [] }); setDirty(true); setCatalogOpen(false); setNotice('已建立本地编辑，填写后保存；当前选用服务保持不变。'); };
  const updateValue = (id: string, value: CatalogScalar) => { editRevision.current++; testEpoch.current++; void cancelTest(); setTestResult(null); setDirty(true); setEditor(current => { if (!current) return null; const values = { ...current.values, [id]: value }; if (id === 'proxy_mode' && value !== 'override') delete values.proxy_url; return { ...current, values, authentication: current.authentication.kind === 'google_owned_grant' && ['oauth_client_mode', 'oauth_client_id', 'google_auth_method'].includes(id) ? { kind: 'none' } : current.authentication }; }); };
  const captureSecret = (id: string, value: string) => { if (secrets.current.get(id) === value) return; if (value) secrets.current.set(id, value); else secrets.current.delete(id); secretRevision.current++; if (value) { editRevision.current++; testEpoch.current++; void cancelTest(); setTestResult(null); setDirty(true); } };
  const title = editor ? typeof editor.values.name === 'string' && editor.values.name ? editor.values.name : serviceLabel(editor.target, catalog) : '';
  const scalar = (id: string) => editor?.values[id] ?? form.projection?.fields.find(row => row.descriptor.id === id)?.resolved.value;
  const oauthMode = scalar('oauth_client_mode');
  const oauthClientId = scalar('oauth_client_id');
  const proxyMode = scalar('proxy_mode'), proxyUrl = scalar('proxy_url');
  const oauthProxy: GoogleInteractiveProxy | null = typeof proxyMode === 'string' ? { mode: proxyMode, ...(proxyMode === 'override' && typeof proxyUrl === 'string' ? { proxyUrl } : {}) } : editor ? googleProxy(editor, configuration) : null;
  const googleContext: GoogleInteractiveContext | null = active && !editorLoading && form.projection?.credentialCapture === 'google_owned_grant' && scalar('google_auth_method') === 'oauth' && (oauthMode === 'application' || oauthMode === 'custom') && oauthProxy ? {
    expectedProfileGeneration: snapshot.profileGeneration, clientMode: oauthMode,
    ...(oauthMode === 'custom' && typeof oauthClientId === 'string' ? { publicClientId: oauthClientId } : {}),
    ...(editor?.authentication.kind === 'google_owned_grant' ? { authorization: editor.authentication.authorization } : {}),
    proxy: oauthProxy,
  } : null;
  const google = useGoogleAuthorization({
    port: store.port, editorKey: editor?.key ?? null, context: googleContext, active,
    onAuthorized: authorization => {
      if (!editor) return;
      const editorKey = editor.key;
      editRevision.current++; testEpoch.current++; void cancelTest(); setTestResult(null); setDirty(true);
      setEditor(current => current?.key === editorKey ? { ...current, authentication: { kind: 'google_owned_grant', authorization } } : current);
      setNotice('Google 授权已完成，当前编辑已更新。请保存配置，再从统一选择器明确选用。');
    },
    onRevoked: result => {
      if (!editor) return;
      const editorKey = editor.key;
      editRevision.current++; testEpoch.current++; void cancelTest(); setTestResult(null); setDirty(true);
      setEditor(current => current?.key === editorKey ? { ...current, authentication: { kind: 'none' } } : current);
      setGoogleRevokeOpen(false); setNotice(result.message); void refresh(); void store.refresh();
    },
  });
  const googleContextKey = JSON.stringify([editor?.key, googleContext]);
  useEffect(() => { setGoogleRevokeOpen(false); }, [googleContextKey]);
  const credential = async (): Promise<ServiceAuthenticationSelection> => {
    if (!editor || !form.projection) throw new Error('尚未取得已核对的配置字段。');
    const kind = form.projection.credentialCapture;
    if (kind === 'none') return { kind: 'none' };
    if (kind === 'unknown') throw new Error('请先确定认证方式。');
    if (kind === 'google_owned_grant') { if (editor.authentication.kind !== 'google_owned_grant') throw new Error('尚未完成此服务的授权；不会替你使用其他认证方式。'); return editor.authentication; }
    if (storedSecret.current?.revision === secretRevision.current) return storedSecret.current.authentication;
    let input: ServiceCredentialInput | null = null;
    if (kind === 'api_key' && secrets.current.has('api_key')) input = { kind, label: String(scalar('name') ?? title), key: secrets.current.get('api_key')! };
    else if (kind === 'baidu_app_secret' && secrets.current.has('app_secret')) input = { kind, label: String(scalar('name') ?? title), appId: String(scalar('app_id') ?? ''), secret: secrets.current.get('app_secret')! };
    else if (kind === 'youdao_app_secret' && secrets.current.has('app_secret')) input = { kind, label: String(scalar('name') ?? title), appKey: String(scalar('app_key') ?? ''), secret: secrets.current.get('app_secret')! };
    else if (kind === 'google_service_account_file' && secrets.current.has('service_account_json')) input = { kind, label: String(scalar('name') ?? title), fileContents: secrets.current.get('service_account_json')! };
    if (!input) return editor.authentication;
    const saved = await store.port.storeServiceCredential({ expectedProfileGeneration: snapshot.profileGeneration, material: input }); const authentication: ServiceAuthenticationSelection = { kind: 'existing_pool', poolId: saved.poolId, poolRevision: saved.poolRevision }; storedSecret.current = { revision: secretRevision.current, authentication }; return authentication;
  };
  const save = async () => {
    if (!editor || saving || editorLoading) return;
    const generation = editorEpoch.current, beforeProfile = snapshot.profileGeneration, revision = editRevision.current; setSaving(true); setError(null);
    try {
      const authentication = await credential();
      if (generation !== editorEpoch.current || profile.current !== beforeProfile) return;
      const receipt = await store.port.saveServiceDraft({ operationId: crypto.randomUUID(), expectedProfileGeneration: beforeProfile, expectedDraftHeadRevision: editor.expectedHead, ...(editor.source ? { editing: editor.source } : {}), target: editor.target, purpose: editor.purpose, values: structuredClone(editor.values), authentication });
      if (generation !== editorEpoch.current || profile.current !== beforeProfile || editRevision.current !== revision) return;
      setDirty(false); secrets.current.clear(); setNotice('服务配置已保存；当前服务、当前句与自动翻译开关保持不变。'); await openSaved(receipt.savedResource, editor.purpose); await refresh(); setRefreshKey(value => value + 1); await store.refresh();
    } catch (reason) { if (generation === editorEpoch.current) setError(errorMessage(reason)); }
    finally { if (profile.current === beforeProfile) setSaving(false); }
  };
  const test = async () => {
    if (!editor?.source || dirty || runningTest.current) return;
    const generation = ++testEpoch.current, operation = { operationId: crypto.randomUUID(), profileGeneration: snapshot.profileGeneration, label: serviceLabel(editor.target, catalog), cancelRequested: false };
    runningTest.current = operation; setTesting(true); setCancellingTest(false); setTestResult('正在测试这项已保存配置…');
    try { const result = await store.port.testSavedService({ expectedProfileGeneration: snapshot.profileGeneration, source: editor.source, purpose: editor.purpose }, operation.operationId); if (testEpoch.current === generation) setTestResult(result.failure ? <Notice error>{result.message}<br/>{result.failure.recovery}<br/><small>{result.failure.code}{result.failure.stage ? ` · ${result.failure.stage}` : ''}</small></Notice> : result.message); }
    catch (reason) { if (testEpoch.current === generation) setTestResult(errorMessage(reason)); }
    finally {
      if (runningTest.current === operation) { runningTest.current = null; setTesting(false); setCancellingTest(false); }
      if (profile.current === operation.profileGeneration) await refresh();
    }
  };
  const fileChanged = async (file?: File) => {
    const generation = editorEpoch.current;
    if (!file || !activeRef.current) return;
    if (file.size === 0 || file.size > 65536) { setError('凭据文件应为非空 JSON，最大64 KiB；尚未读取文件内容。'); return; }
    try { const text = await file.text(); if (generation !== editorEpoch.current || !activeRef.current) return; captureSecret('service_account_json', text); setNotice('已选择凭据文件；保存时才写入系统密钥库。'); } catch { if (generation === editorEpoch.current) setError('无法读取所选凭据文件。'); }
    finally { if (fileInput.current) fileInput.current.value = ''; }
  };
  const scope: ServiceScope = scopeKey === 'global' ? { kind: 'global' } : { kind: 'game', gameId: scopeKey.slice(5) };
  const selector = (purpose: CatalogPurpose) => <ServiceSelector showPurposeControl port={store.port} profileGeneration={snapshot.profileGeneration} scope={scope} purpose={purpose} refreshKey={refreshKey} onConfigure={() => request({ kind: 'catalog', purpose })} onSelected={() => { void refresh(); void store.refresh(); }}/>;
  const setFallback = (fallback: SavedServiceRef[]) => { if (!editor) return; setEditor({ ...editor, fallback, values: { ...editor.values, fallback: JSON.stringify(fallback) } }); editRevision.current++; testEpoch.current++; void cancelTest(); setDirty(true); setTestResult(null); };
  const fallbackRows = editor ? rawRows.filter(row => row.purpose === editor.purpose) : [];
  const fallbackControl = editor ? <fieldset className={styles.fallbackPicker} disabled={saving}><legend>备用服务</legend><p>按列表顺序尝试已保存版本；不会自动选入当前主服务。</p><ol>{editor.fallback.map((source, index) => <li key={savedServiceKey(source)}><span>{fallbackRows.find(row => savedServiceKey(row.source) === savedServiceKey(source))?.displayName ?? '已保存的备用版本'}</span><Button label={`上移备用服务${index + 1}`} disabled={index === 0 || saving} onPress={() => { const next = [...editor.fallback]; [next[index - 1], next[index]] = [next[index], next[index - 1]]; setFallback(next); }}>上移</Button><Button label={`移除备用服务${index + 1}`} disabled={saving} onPress={() => setFallback(editor.fallback.filter((_, position) => position !== index))}>移除</Button></li>)}</ol>{fallbackRows.filter(row => !editor.fallback.some(source => savedServiceKey(source) === savedServiceKey(row.source))).map(row => <div key={savedServiceKey(row.source)}><Button disabled={saving || row.eligibility.kind !== 'selectable'} onPress={() => setFallback([...editor.fallback, row.source])}>添加{row.displayName}</Button>{row.eligibility.kind === 'blocked' ? <small>{row.eligibility.reasons.map(reason => reason.message).join('；')}</small> : null}</div>)}</fieldset> : null;
  const fieldActions: Record<string, GuidedServiceFieldAction> = {};
  if (form.projection?.credentialCapture === 'google_service_account_file') fieldActions.service_account_json = { enabled: true, onPress: () => fileInput.current?.click(), label: '选择凭据文件…' };
  const googleControl = <div data-google-authorization-state={google.operation?.state ?? google.projection?.state ?? 'unconfigured'}>
    <Button disabled={saving || google.loading || google.pending || google.revoking || !google.projection?.canLogin || google.operation?.state === 'cleanup_pending'} onPress={() => void google.start()}>登录并授权Google</Button>
    {google.pending ? <Button disabled={!google.operation?.canCancel} onPress={() => void google.cancel()}>{google.operation?.canCancel ? '取消授权' : '正在安全收尾…'}</Button> : null}
    {google.projection?.canLogout ? <Button disabled={saving || google.pending || google.revoking} intent="quiet" onPress={() => setGoogleRevokeOpen(true)}>解除授权…</Button> : null}
    <p role="status" className={styles.help}>{!oauthProxy ? '尚未核对原连接的网络策略，不能启动授权；请重新读取服务配置。' : google.loading ? '正在核对 Google 授权状态…' : google.operation?.message ?? google.projection?.message ?? '请先确定 Google 登录客户端。'}</p>
    {google.error ? <Notice error>{google.error}<Button disabled={google.pending || google.revoking} onPress={google.reload}>重新读取授权状态</Button></Notice> : null}
  </div>;
  return <div className={styles.host} data-service-workspace data-guided-service-page>
    <div className={styles.scroll} data-ui-scroll="configuration">{header ?? <header className={styles.guidedHeading}><h1>翻译服务</h1><p>选择你要使用的服务，再填写必要的信息。</p></header>}
      {error ? <Notice error>{error} 当前输入已保留。<Button intent="quiet" onPress={() => void refresh()}>重新读取</Button></Notice> : null}
      {notice ? <p role="status" className={styles.help}>{notice}</p> : null}
      <section className={styles.savedServices} data-guided-section="saved"><div className={styles.serviceHeading}><h2 ref={listHeading} tabIndex={-1}>已添加的服务</h2><Button disabled={!catalog || saving} onPress={() => request({ kind: 'catalog', purpose: 'translation' })}>添加服务…</Button></div>
        {loading ? <p role="status" className={styles.help}>正在读取服务…</p> : null}
        {!loading && !error && managementGroups.length === 0 ? <p className={styles.help}>还没有添加服务。从熟悉的服务名称开始。</p> : null}
        <div className={styles.guidedServiceRows}>{managementGroups.map(group => <button key={group.key} className={styles.guidedSavedRow} onClick={() => { if (group.editor) request({ kind: 'edit', ...group.editor }); }} disabled={saving || editorLoading || loading || configuration?.profileGeneration !== snapshot.profileGeneration || !group.editor} data-guided-saved={group.logicalResourceId}><span><strong>{group.displayName}</strong><small>{backendNames[group.backend]}</small>{group.sources.map(source => <span key={serviceManagementSourceKey(source.source)} data-service-management-source={serviceManagementSourceKey(source.source)} data-service-management-draft={source.draft} data-service-management-current={source.purposes.some(value => value.current)}><small>{source.draft ? '已保存草稿' : '当前选用版本'}{source.draft && source.purposes.some(value => value.current) ? ' · 当前选用' : ''}</small>{source.purposes.length ? source.purposes.map(value => <small key={value.purpose} data-service-management-purpose={value.purpose}>{guidedPurposeNames[value.purpose]}{value.current ? '（当前选用）' : value.bound ? '（已绑定）' : '（已声明）'}：{serviceManagementState(value.row)}</small>) : <small>用途待核验，重新读取后再编辑。</small>}{[...new Set((source.purposes.length ? source.purposes.map(value => value.row) : source.rows).flatMap(row => row.eligibility.kind === 'blocked' ? row.eligibility.reasons.map(reason => reason.message) : []))].map(reason => <small key={reason}>{reason}</small>)}</span>)}</span><span>{group.sources.some(source => (source.purposes.length ? source.purposes.map(value => value.row) : source.rows).some(row => row.eligibility.kind === 'blocked' && row.eligibility.reasons.some(reason => reason.code === 'legacy_requires_guided_configuration'))) ? '旧版配置' : '已保存'}</span><span>{group.editor ? '编辑' : '用途待核验'}</span></button>)}</div>
      </section>
      {unmapped ? <Notice>{unmapped}</Notice> : null}
      {editorLoading ? <p role="status">正在读取所选配置…</p> : null}
      {active && editor ? <div className={styles.guidedEditorHost}>{editor.readonlyReason ? <Notice>{editor.readonlyReason}</Notice> : null}{configuration && editor.expectedHead !== configuration.draftHeadRevision && dirty ? <Notice error>已保存版本已变化。本地输入已保留；保存仍按原版本核对，不会覆盖其他变更。</Notice> : null}<GuidedServiceEditor actionsPlacement="page-footer" resourceKey={editor.key} title={title} subtitle={editor.target.kind === 'offline' ? '仅在本机使用' : '需要时才发送测试样句'} projection={form.projection} values={editor.values} loading={form.loading} error={form.error} busy={saving} canSave={dirty && !editor.readonlyReason} canTest={!!editor.source && !dirty && !testing && !editor.readonlyReason} onChange={updateValue} onSecretChange={captureSecret} onSave={() => void save()} onTest={() => void test()} testing={testing} cancellingTest={cancellingTest} onCancelTest={() => void cancelTest()} onCancel={() => request({ kind: 'cancel' })} onChangeService={() => request({ kind: 'catalog', purpose: editor.purpose })} fieldActions={fieldActions} fieldControls={{ fallback: fallbackControl, oauth_login: googleControl }} result={testing && runningTest.current ? `正在完成“${runningTest.current.label}”的已保存配置测试；取消后以实际终态为准。` : testResult ?? (dirty ? '尚未保存；测试只使用已保存版本。' : '保存不切换当前服务；从统一选择器明确选用。')}/>{editor.shared.length ? <p className={styles.help}>此配置与{editor.shared.length}项模型共享连接；保存前由后端核对影响范围。</p> : null}<details className={styles.disclosure}><summary>共用账号与凭据</summary><div className={styles.disclosureBody}><Select label="使用已有系统凭据池" value={editor.authentication.kind === 'existing_pool' ? `${editor.authentication.poolId}@${editor.authentication.poolRevision}` : ''} onChange={value => { const pool = snapshot.credentialPools.find(item => `${item.poolId}@${item.revision}` === value); if (pool) { secrets.current.clear(); storedSecret.current = null; secretRevision.current++; testEpoch.current++; void cancelTest(); setTestResult(null); setEditor({ ...editor, key: crypto.randomUUID(), authentication: { kind: 'existing_pool', poolId: pool.poolId, poolRevision: pool.revision } }); setDirty(true); editRevision.current++; } }} options={[{ value: '', label: '尚未选择已有凭据' }, ...snapshot.credentialPools.map(pool => ({ value: `${pool.poolId}@${pool.revision}`, label: `${typeof pool.metadata.label === 'string' ? pool.metadata.label : '系统凭据池'} · v${pool.revision}` }))]}/><CredentialRevocationPanel snapshot={snapshot} store={store}/></div></details></div> : null}
      <details className={styles.disclosure} data-configuration-section="translation"><summary>翻译设置<small>选择已保存服务与作用范围</small></summary><div className={styles.disclosureBody}><Select label="服务作用范围" value={scopeKey} onChange={setScopeKey} options={[{ value: 'global', label: '全局默认' }, ...snapshot.games.map(game => ({ value: `game:${game.gameId}`, label: game.displayName }))]}/>{selector('translation')}<p className={styles.help}>保存配置不会替你选用。主服务与对照服务的版本、可用状态和继承关系由已保存配置决定。</p></div></details>
      <details className={styles.disclosure} data-configuration-section="learning"><summary>学习设置（可选）<small>句子精读与语境查词按需使用</small></summary><div className={styles.disclosureBody}><section><h3>句子精读</h3>{selector('grammar')}<Button onPress={() => request({ kind: 'catalog', purpose: 'grammar' })}>添加精读服务…</Button></section><section><h3>语境查词</h3>{selector('context_word')}<Button onPress={() => request({ kind: 'catalog', purpose: 'context_word' })}>添加查词服务…</Button></section></div></details>
      {configuration?.draft.kind === 'legacy' ? <LegacyMaintenance key={`${snapshot.profileGeneration}:${resetKey}`} configuration={configuration.draft.configuration} snapshot={snapshot} store={store} onDirty={setLegacyDirty}/> : configuration?.draft.kind === 'service_graph' ? <details className={styles.disclosure}><summary>术语与共用资料</summary><div className={styles.disclosureBody}><p className={styles.help}>已保存的{configuration.draft.configuration.termSets.length}份术语表继续保留；此版本的资料维护入口尚未接通。</p><Button disabled>维护术语与凭据池</Button></div></details> : null}
    </div>
    <footer className={styles.footer}><span role="status">{dirty ? '有未保存的编辑；当前服务保持不变。' : '保存与选用分别操作。'}<br/><small>{configuration ? `草稿 v${configuration.draftHeadRevision} · 已应用 v${configuration.appliedHeadRevision}` : '正在核对配置版本'}</small></span><div><Button disabled={!editor || saving} onPress={() => request({ kind: 'cancel' })}>取消</Button><Button intent="primary" pending={saving} disabled={!editor || !dirty || editorLoading || !!editor.readonlyReason || form.loading || !!form.error} onPress={() => void save()}>保存配置</Button></div></footer>
    {catalog ? <ServiceCatalogPicker catalog={catalog} open={catalogOpen} initialPurpose={catalogPurpose} busy={saving} onClose={() => setCatalogOpen(false)} onSelect={add}/> : null}
    <Sheet title="放弃未保存的编辑？" description="已保存服务、当前选择和运行状态保持不变。未保存的密钥会清除。" open={next !== null} onClose={() => setNext(null)} footer={<><Button onPress={() => setNext(null)}>继续编辑</Button><Button intent="danger" onPress={() => { if (next) { clearEdit(); perform(next); } }}>放弃并继续</Button></>}><p>当前输入尚未保存。</p></Sheet>
    <Sheet title="解除 Google 授权？" description="此操作立即撤销本地使用权限；其他共用服务也会失去这项授权。不会删除阅读历史。" open={googleRevokeOpen} onClose={() => { if (!google.revoking) setGoogleRevokeOpen(false); }} footer={<><Button disabled={google.revoking} onPress={() => setGoogleRevokeOpen(false)}>取消</Button><Button intent="danger" pending={google.revoking} disabled={!google.projection?.canLogout || google.pending} onPress={() => void google.revoke()}>确认解除授权</Button></>}>
      <p>当前配置档有 {google.projection?.affectedServiceCount ?? 0} 项已保存服务引用这项授权。</p>
      <p>本地撤销与 Google 远端确认分别记录；网络失败时不会把远端撤销显示为已确认。</p>
      {google.error ? <Notice error>{google.error}</Notice> : null}
    </Sheet>
    <input type="file" accept="application/json,.json" ref={fileInput} hidden aria-label="选择Google服务账号凭据文件" onChange={event => void fileChanged(event.target.files?.[0])}/>
  </div>;
}
