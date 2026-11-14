import { ConfigurationRow as FormRow } from './ConfigurationFields';
import { useState } from 'react';
import type { AppSnapshot, ConnectionRuntimeView, PurposeImpact, PurposeRuntimeView } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { Button, Sheet } from '../../ui/primitives/Controls';
import { Group, Notice, StatusLabel } from '../../ui/patterns/Patterns';
import { purposeNames } from './PurposeEditor';
import styles from './styles.module.css';
import { purposeRuntimeStatus } from './runtime-status';

type Selection = { kind: 'connection'; connectionId: string } | { kind: 'purpose'; scopeKey: string; purpose: string };

export function AppliedPurposeImpact({ snapshot, impact }: { snapshot: AppSnapshot; impact: PurposeImpact }) {
  const applied = snapshot.configuration.applied?.kind === 'legacy' ? snapshot.configuration.applied.configuration : null;
  const binding = applied?.bindings.find(item => item.bindingId === impact.bindingId);
  const models = impact.engineIds.map(id => binding?.engines.find(engine => engine.engineId === id)).flatMap(engine => {
    const model = applied?.models.find(item => item.modelId === engine?.modelId);
    return model ? [model.displayName] : [];
  });
  const games = impact.affectedGameIds.map(id => snapshot.games.find(game => game.gameId === id)?.displayName ?? '已移出库的游戏');
  return <li>
    <strong>{impact.scopeKey === 'global' ? '全局默认' : snapshot.games.find(game => `game:${game.gameId}` === impact.scopeKey)?.displayName ?? '游戏专用'} · {purposeNames[impact.purpose] ?? impact.purpose}</strong>
    <p>{!impact.enabledIntent ? '已应用意图未启用' : !impact.runtimeEnabled ? '此用途已单独停用' : '用途未单独停用；仍受连接和凭据限制'}{models.length ? ` · 引擎：${models.join('、')}` : ''}</p>
    <small>{impact.scopeKey === 'global' ? '含手动输入；' : ''}{games.length ? `关联游戏：${games.join('、')}` : '当前没有关联的已入库游戏'}</small>
  </li>;
}

function ConnectionRuntimeControls({ snapshot, store, view }: { snapshot: AppSnapshot; store: ApplicationStore; view: ConnectionRuntimeView }) {
  const action = useAction();
  const [review, setReview] = useState<ConnectionRuntimeView | null>(null);
  const applied = snapshot.configuration.applied?.kind === 'legacy' ? snapshot.configuration.applied.configuration : null;
  const connection = applied?.connections.find(item => item.connectionId === view.connectionId);
  const stale = review !== null && (review.configRevisionId !== view.configRevisionId || review.controlRevision !== view.controlRevision || !view.suspended);
  return <Group title="已应用连接 · 即时运行控制" note="只控制已应用版本；不会保存或应用正在编辑的草稿。开放本地许可不表示服务已连通。">
    <FormRow label="正在控制"><span>{connection?.displayName ?? '已应用连接'} · 已应用 v{snapshot.configuration.appliedHeadRevision}</span></FormRow>
    <FormRow label="运行许可"><StatusLabel>{view.suspended ? '已即时停用' : view.availabilityIntent ? '连接许可开放' : '已应用版本未允许使用'}</StatusLabel></FormRow>
    <div className={styles.runtimeImpact} aria-label="此连接的已应用影响"><strong>共享引用与影响</strong>{view.affectedPurposes.length ? <ul>{view.affectedPurposes.map(impact => <AppliedPurposeImpact key={impact.bindingId} snapshot={snapshot} impact={impact}/>)}</ul> : <p>当前已应用用途没有引用这个连接。</p>}</div>
    {action.error ? <Notice error>{action.error} 草稿编辑已保留。</Notice> : null}
    <FormRow label="立即操作">{view.suspended ? <Button disabled={!view.availabilityIntent} onPress={() => { action.clearError(); setReview(structuredClone(view)); }} pending={action.pending}>检查并恢复连接…</Button> : <Button intent="danger" pending={action.pending} onPress={() => void action.run(() => store.command({ kind: 'set_connection_suspended', payload: { connectionId: view.connectionId, suspended: true, expectedControlRevision: view.controlRevision } }))}>立即停用连接</Button>}</FormRow>
    {!view.availabilityIntent ? <Notice>已应用版本未允许使用此连接；先明确应用可用意图，再恢复运行许可。</Notice> : null}
    <Sheet title="恢复已应用连接？" description="仅恢复下面这些已应用引用的连接许可；不会采用草稿中的模型、地址、参数或启用开关。其他用途、连接或凭据的阻断仍然有效。" open={review !== null} onClose={() => setReview(null)} footer={<><Button onPress={() => setReview(null)}>取消</Button><Button intent="primary" pending={action.pending} disabled={stale} onPress={() => { if (review) void action.run(async () => { await store.command({ kind: 'set_connection_suspended', payload: { connectionId: review.connectionId, suspended: false, expectedControlRevision: review.controlRevision } }); setReview(null); }); }}>恢复此已应用连接</Button></>}>
      {stale ? <Notice error>已应用版本或运行许可已变化，请关闭后重新核对。</Notice> : null}
      {action.error ? <Notice error>{action.error} 连接没有被界面自动恢复。</Notice> : null}
      <p>{connection?.displayName} · 已应用 v{snapshot.configuration.appliedHeadRevision}</p>
      <ul className={styles.impactList}>{review?.affectedPurposes.map(impact => <AppliedPurposeImpact key={impact.bindingId} snapshot={snapshot} impact={impact}/>)}</ul>
    </Sheet>
  </Group>;
}

function PurposeRuntimeControls({ snapshot, store, view }: { snapshot: AppSnapshot; store: ApplicationStore; view: PurposeRuntimeView }) {
  const action = useAction();
  const [review, setReview] = useState<PurposeRuntimeView | null>(null);
  const applied = snapshot.configuration.applied?.kind === 'legacy' ? snapshot.configuration.applied.configuration : null;
  const binding = applied?.bindings.find(item => item.bindingId === view.bindingId);
  const blocked = view.blockedConnectionIds.map(id => applied?.connections.find(connection => connection.connectionId === id)?.displayName ?? '已应用连接');
  const stale = review !== null && (review.configRevisionId !== view.configRevisionId || review.authorizationEpoch !== view.authorizationEpoch || view.runtimeEnabled);
  const status = purposeRuntimeStatus(view);
  const impact: PurposeImpact = { scopeKey: view.scopeKey, purpose: view.purpose, bindingId: view.bindingId, enabledIntent: view.enabledIntent, runtimeEnabled: view.runtimeEnabled, affectedGameIds: view.affectedGameIds, engineIds: binding?.engines.filter(engine => engine.enabled).map(engine => engine.engineId) ?? [] };
  return <Group title="已应用用途 · 即时运行控制" note="停用立即阻止新请求并取消相关在途请求；原文与本地词典仍可使用。运行许可与草稿启用意图分开。">
    <FormRow label="正在控制"><span>{purposeNames[view.purpose] ?? view.purpose} · 已应用 v{snapshot.configuration.appliedHeadRevision}</span></FormRow>
    <FormRow label="实际许可"><StatusLabel>{status}</StatusLabel></FormRow>
    {blocked.length ? <Notice>连接或凭据阻断：{blocked.join('、')}。恢复用途不会解除这些阻断。</Notice> : null}
    <ul className={styles.impactList}><AppliedPurposeImpact snapshot={snapshot} impact={impact}/></ul>
    {action.error ? <Notice error>{action.error} 草稿编辑已保留。</Notice> : null}
    <FormRow label="立即操作">{view.runtimeEnabled ? <Button intent="danger" pending={action.pending} onPress={() => void action.run(() => store.command({ kind: 'set_purpose_runtime_enabled', payload: { scopeKey: view.scopeKey, purpose: view.purpose, enabled: false, expectedAuthorizationEpoch: view.authorizationEpoch } }))}>立即停用此用途</Button> : <Button disabled={!view.enabledIntent} pending={action.pending} onPress={() => { action.clearError(); setReview(structuredClone(view)); }}>检查并恢复用途…</Button>}</FormRow>
    {!view.enabledIntent ? <Notice>已应用版本未启用此用途；先明确应用启用意图，再恢复运行许可。</Notice> : null}
    <Sheet title="恢复已应用用途？" description="只恢复这一用途的独立运行许可。使用已应用版本；当前未保存、未应用的编辑不参与本次操作。已应用启用意图不满足时，后端将拒绝恢复。连接和凭据的阻断仍然保留。" open={review !== null} onClose={() => setReview(null)} footer={<><Button onPress={() => setReview(null)}>取消</Button><Button intent="primary" pending={action.pending} disabled={stale} onPress={() => { if (review) void action.run(async () => { await store.command({ kind: 'set_purpose_runtime_enabled', payload: { scopeKey: review.scopeKey, purpose: review.purpose, enabled: true, expectedAuthorizationEpoch: review.authorizationEpoch } }); setReview(null); }); }}>恢复此已应用用途</Button></>}>
      {stale ? <Notice error>已应用版本或运行许可已变化，请关闭后重新核对。</Notice> : null}
      {action.error ? <Notice error>{action.error} 此次恢复未完成。</Notice> : null}
      <ul className={styles.impactList}><AppliedPurposeImpact snapshot={snapshot} impact={impact}/></ul>
      {blocked.length ? <Notice>仍被阻断的连接：{blocked.join('、')}。</Notice> : null}
    </Sheet>
  </Group>;
}

export function RuntimeConfigurationPanel({ snapshot, store, selection, onGlobal }: { snapshot: AppSnapshot; store: ApplicationStore; selection: Selection; onGlobal?: () => void }) {
  if (selection.kind === 'connection') {
    const view = snapshot.configuration.runtimeConnections.find(item => item.connectionId === selection.connectionId);
    return view ? <ConnectionRuntimeControls key={view.connectionId} snapshot={snapshot} store={store} view={view}/> : <Notice>这个连接尚未应用，没有可操作的运行许可。保存草稿或测试不会使它运行。</Notice>;
  }
  const view = snapshot.configuration.runtimePurposes.find(item => item.scopeKey === selection.scopeKey && item.purpose === selection.purpose);
  const inherited = selection.scopeKey !== 'global' && snapshot.configuration.runtimePurposes.some(item => item.scopeKey === 'global' && item.purpose === selection.purpose);
  return view ? <PurposeRuntimeControls key={`${view.scopeKey}:${view.purpose}`} snapshot={snapshot} store={store} view={view}/> : inherited ? <Notice>当前已应用版本没有此游戏的独立用途许可；运行许可由全局用途管理。<Button intent="quiet" onPress={onGlobal}>查看全局运行许可</Button></Notice> : <Notice>这个用途尚未应用，没有可操作的运行许可。先在草稿中配置，再明确应用。</Notice>;
}
