import { useCallback, useEffect, useRef, useState } from 'react';
import type { CatalogPurpose, SavedServiceProjection, SavedServiceRef, ServiceScope, ServiceSelectionReceipt } from '../../bridge/contracts.generated';
import type { ApplicationPort } from '../../bridge/application-port';
import { errorMessage } from '../../bridge/application-port';
import { Button, Sheet, Switch } from '../../ui/primitives/Controls';
import { Icon } from '../../ui/icons/Icon';
import { guidedPurposeNames } from './ServiceCatalogPicker';
import styles from './service-selector.module.css';

export interface ServiceSelectorProps {
  port: ApplicationPort;
  profileGeneration: number;
  scope: ServiceScope;
  purpose: CatalogPurpose;
  onConfigure: () => void;
  refreshKey?: string | number;
  appearance?: 'standard' | 'hud' | 'session' | 'manual';
  onSelected?: (receipt: ServiceSelectionReceipt) => void;
  showPurposeControl?: boolean;
}
export function savedServiceKey(value: SavedServiceRef) {
  return value.kind === 'http' ? JSON.stringify(['http', value.source.revisionId, value.source.modelId]) : JSON.stringify(['offline', value.source.revisionId, value.source.configId]);
}
function sameRef(a: SavedServiceRef | null, b: SavedServiceRef | null) { return a !== null && b !== null && savedServiceKey(a) === savedServiceKey(b); }
const backendNames = { http_llm: '大模型服务', http_translation: '云端翻译', platform_native: '系统翻译', local_worker: '本机离线翻译' };

/** Saved resources, execution eligibility and inheritance are authoritative Rust projections. */
export function ServiceSelector({ port, profileGeneration, scope, purpose, onConfigure, refreshKey = 0, onSelected, showPurposeControl = false, appearance = 'standard' }: ServiceSelectorProps) {
  const [open, setOpen] = useState(false), [viewScope, setViewScope] = useState(scope);
  const [projection, setProjection] = useState<SavedServiceProjection | null>(null), [loading, setLoading] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const epoch = useRef(0), mutationEpoch = useRef(0);
  const scopeKey = scope.kind === 'game' ? `game:${scope.gameId}` : 'global';
  const viewScopeKey = viewScope.kind === 'game' ? `game:${viewScope.gameId}` : 'global';
  const context = useRef({ profileGeneration, scopeKey, purpose }); context.current = { profileGeneration, scopeKey, purpose };
  useEffect(() => () => { epoch.current++; mutationEpoch.current++; }, []);
  useEffect(() => { mutationEpoch.current++; setBusy(false); setViewScope(scope); setProjection(null); setError(null); }, [scopeKey, purpose, profileGeneration]);
  const load = useCallback(async () => {
    const generation = ++epoch.current; setLoading(true); setError(null);
    try {
      const next = await port.savedServices({ expectedProfileGeneration: profileGeneration, scope: viewScope, purpose });
      if (epoch.current !== generation) return;
      if (next.profileGeneration !== profileGeneration || next.selection.requestedScope.kind !== viewScope.kind || (next.selection.requestedScope.kind === 'game' && (viewScope.kind !== 'game' || next.selection.requestedScope.gameId !== viewScope.gameId))) throw new Error('服务列表的范围已变化，请重新打开。');
      if (next.rows.some(row => row.purpose !== purpose)) throw new Error('服务用途不匹配，未采用返回列表。');
      setProjection(next); return next;
    } catch (reason) { if (epoch.current === generation) setError(errorMessage(reason)); }
    finally { if (epoch.current === generation) setLoading(false); }
  }, [port, profileGeneration, viewScopeKey, purpose]);
  useEffect(() => { void load(); return () => { epoch.current++; }; }, [load, refreshKey, open]);
  const dismiss = () => { setOpen(false); if (viewScopeKey !== scopeKey) { setViewScope(scope); setProjection(null); } };
  const configure = () => { dismiss(); onConfigure(); };
  const current = projection?.rows.find(row => sameRef(row.source, projection.selection.main));
  const inherited = projection?.selection.requestedScope.kind === 'game' && projection.selection.effectiveScope.kind === 'global';
  const choose = async (main: SavedServiceRef, comparisons: SavedServiceRef[], close: boolean) => {
    if (!projection || busy) return;
    const generation = ++mutationEpoch.current, before = { ...context.current }; setBusy(true); setError(null);
    try {
      const receipt = await port.selectSavedServices({ operationId: crypto.randomUUID(), expectedProfileGeneration: profileGeneration, scope: viewScope, purpose, expectedSelectionRevision: projection.selection.selectionRevision, expectedAppliedHeadRevision: projection.appliedHeadRevision, main, comparisons });
      if (mutationEpoch.current !== generation || context.current.profileGeneration !== before.profileGeneration || context.current.scopeKey !== before.scopeKey || context.current.purpose !== before.purpose) return;
      const loaded = await load(); onSelected?.(receipt); if (close && loaded) dismiss();
    } catch (reason) { if (mutationEpoch.current === generation) setError(errorMessage(reason)); }
    finally { if (mutationEpoch.current === generation) setBusy(false); }
  };
  const setEnabled = async (enabled: boolean) => {
    if (!projection || busy || loading) return;
    const generation = ++mutationEpoch.current; setBusy(true); setError(null);
    try { const receipt = await port.setServicePurposeEnabled({ operationId: crypto.randomUUID(), expectedProfileGeneration: profileGeneration, scope: viewScope, purpose, expectedSelectionRevision: projection.selection.selectionRevision, expectedAppliedHeadRevision: projection.appliedHeadRevision, enabled }); if (generation !== mutationEpoch.current) return; await load(); onSelected?.(receipt); }
    catch (reason) { if (generation === mutationEpoch.current) setError(errorMessage(reason)); }
    finally { if (generation === mutationEpoch.current) setBusy(false); }
  };
  const inherit = async () => {
    if (!projection || viewScope.kind !== 'game' || busy) return;
    const generation = ++mutationEpoch.current; setBusy(true); setError(null);
    try { const receipt = await port.restoreServiceInheritance({ operationId: crypto.randomUUID(), expectedProfileGeneration: profileGeneration, gameId: viewScope.gameId, purpose, expectedSelectionRevision: projection.selection.selectionRevision, expectedAppliedHeadRevision: projection.appliedHeadRevision }); if (generation !== mutationEpoch.current) return; await load(); onSelected?.(receipt); }
    catch (reason) { if (generation === mutationEpoch.current) setError(errorMessage(reason)); }
    finally { if (generation === mutationEpoch.current) setBusy(false); }
  };
  const compact = appearance === 'hud' || appearance === 'manual';
  return <div className={`${styles.host} ${appearance === 'hud' ? styles.hud : appearance === 'session' ? styles.session : appearance === 'manual' ? styles.manual : ''}`} data-service-selector data-service-appearance={appearance} data-service-purpose={purpose}>
    <button className={styles.trigger} disabled={busy} aria-label={`${guidedPurposeNames[purpose]}服务：${current?.displayName ?? (projection?.selection.main ? '已选服务（已保存版本）' : '尚未选择')}${error ? '，读取失败，打开查看' : ''}`} aria-busy={loading || busy} aria-haspopup="dialog" aria-expanded={open} onClick={() => { setViewScope(scope); setOpen(true); }}><Icon name="languages"/><span><small>{guidedPurposeNames[purpose]}服务</small><strong>{current?.displayName ?? (projection?.selection.main ? '已选服务（已保存版本）' : loading ? '读取服务…' : '选择服务…')}</strong></span><Icon name="chevron-down" size={14}/></button>
    {!compact ? <span className={styles.source}>{inherited ? '继承全局默认' : projection?.selection.requestedScope.kind === 'game' ? '本游戏专用' : '全局默认'}</span> : null}
    {showPurposeControl && !compact ? <div className={styles.purposeControl} aria-busy={busy || loading}><Switch label={`启用${guidedPurposeNames[purpose]}`} selected={projection?.selection.purposeEnabled ?? false} disabled={!projection} readOnly={loading || busy} onChange={enabled => void setEnabled(enabled)} description="只改变该范围的用途许可；当前会话自动翻译开关独立控制。"/></div> : null}
    {!open && error && !compact ? <span className={styles.error} role="status">{error}<Button intent="quiet" onPress={() => void load()}>重试</Button></span> : null}
    <Sheet title={`选择${guidedPurposeNames[purpose]}服务`} description="用于后续台词；当前句、历史和自动翻译开关保持原状态。保存或测试配置不会替你切换服务。" open={open} onClose={() => { if (!busy) dismiss(); }} footer={<><Button disabled={busy} onPress={configure}>配置服务…</Button><Button disabled={busy} onPress={dismiss}>关闭</Button></>}>
      {scope.kind === 'game' ? <div className={styles.scopes}><Button pressed={viewScope.kind === 'game'} disabled={busy} onPress={() => setViewScope(scope)}>本游戏</Button><Button pressed={viewScope.kind === 'global'} disabled={busy} onPress={() => setViewScope({ kind: 'global' })}>全局默认</Button></div> : null}
      {error ? <p role="alert" className={styles.error}>{error}<Button onPress={() => void load()}>重新读取</Button></p> : null}
      {loading ? <p role="status">正在读取已保存服务…</p> : null}
      {!loading && projection?.rows.length === 0 ? <p className={styles.empty}>还没有已保存的服务。先添加并保存配置，再从这里明确选用。<Button onPress={configure}>配置服务…</Button></p> : null}
      <div className={styles.rows}>{projection?.rows.map(row => {
        const selected = sameRef(row.source, projection.selection.main), comparison = projection.selection.comparisons.some(item => sameRef(item, row.source)), blocked = row.eligibility.kind === 'blocked';
        return <div key={savedServiceKey(row.source)} className={styles.row} data-saved-service={row.logicalResourceId} data-service-eligibility={row.eligibility.kind}>
          <div><strong>{row.displayName}</strong><small>{backendNames[row.backend]} · {row.verification.kind === 'passed' ? '本项测试通过' : row.verification.kind === 'partially_tested' ? `已测凭据 ${row.verification.passedCredentialCount}/${row.verification.totalCredentialCount}` : row.verification.kind === 'failed' ? '本项测试失败' : row.verification.kind === 'stale' ? '设置已变化，需重测' : '尚未测试'}</small>{row.eligibility.kind === 'blocked' ? <ul>{row.eligibility.reasons.map((reason, index) => <li key={index}>{reason.message}</li>)}</ul> : null}</div>
          <div className={styles.rowActions}><Button disabled={busy || loading || blocked} pressed={selected} onPress={() => void choose(row.source, projection.selection.comparisons.filter(item => !sameRef(item, row.source)), true)}>{selected ? '当前主服务' : '设为主服务'}</Button><Button disabled={busy || loading || blocked || selected || !projection.selection.main} pressed={comparison} onPress={() => { if (projection.selection.main) void choose(projection.selection.main, comparison ? projection.selection.comparisons.filter(item => !sameRef(item, row.source)) : [...projection.selection.comparisons, row.source], false); }}>{comparison ? '取消对照' : '作为对照'}</Button></div>
        </div>;
      })}</div>
      {viewScope.kind === 'game' ? <div className={styles.inheritance}><span>{inherited ? '当前继承全局默认' : '当前使用本游戏的选择'}</span><Button disabled={busy || loading || !projection} onPress={() => void inherit()}>恢复继承全局</Button></div> : null}
    </Sheet>
  </div>;
}
