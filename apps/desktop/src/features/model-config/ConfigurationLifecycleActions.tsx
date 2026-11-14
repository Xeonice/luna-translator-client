import { useState } from 'react';
import type { AppSnapshot, Configuration, Connection, Model } from '../../bridge/contracts.generated';
import { Button, Sheet, TextField } from '../../ui/primitives/Controls';
import { Group, Notice } from '../../ui/patterns/Patterns';
import { ConfigurationRow as FormRow, ConfigurationSwitch } from './ConfigurationFields';
import { purposeNames } from './PurposeEditor';
import styles from './styles.module.css';

type Entity = { kind: 'connection'; value: Connection } | { kind: 'model'; value: Model };
type Props = { configuration: Configuration; snapshot: AppSnapshot; entity: Entity; onChange: (value: Configuration) => void; onSelect: (id: string | null) => void };

function modelIds(configuration: Configuration, entity: Entity): Set<string> {
  return new Set(entity.kind === 'model' ? [entity.value.modelId] : configuration.models.filter(model => model.connectionId === entity.value.connectionId).map(model => model.modelId));
}

/** These controls edit the draft only. Saving/validation remains the Rust command. */
export function ConfigurationLifecycleActions({ configuration, snapshot, entity, onChange, onSelect }: Props) {
  const [review, setReview] = useState<{ configuration: Configuration; entity: Entity } | null>(null);
  const affectedModels = modelIds(configuration, entity);
  const purposes = configuration.bindings.filter(binding => binding.engines.some(engine => affectedModels.has(engine.modelId)));
  const applied = snapshot.configuration.applied?.kind === 'legacy' ? snapshot.configuration.applied.configuration : null;
  const appliedReferences = applied?.bindings.filter(binding => {
    const appliedIds = modelIds(applied, entity);
    return binding.engines.some(engine => appliedIds.has(engine.modelId));
  }) ?? [];
  const stale = review !== null && review.configuration !== configuration;
  const remove = () => {
    if (!review || stale) return;
    const removedModels = modelIds(configuration, review.entity);
    const removedTemplates = new Set(configuration.templates.filter(template => typeof template.compatibility.modelConfigurationId === 'string' && removedModels.has(template.compatibility.modelConfigurationId)).map(template => template.templateId));
    const next: Configuration = {
      ...configuration,
      connections: review.entity.kind === 'connection' ? configuration.connections.filter(connection => connection.connectionId !== review.entity.value.connectionId) : configuration.connections,
      models: configuration.models.filter(model => !removedModels.has(model.modelId)).map(model => ({ ...model, defaultTemplates: Object.fromEntries(Object.entries(model.defaultTemplates).filter(([, id]) => !removedTemplates.has(id))) })),
      templates: configuration.templates.filter(template => !removedTemplates.has(template.templateId)),
      bindings: configuration.bindings.map(binding => {
        const affected = binding.engines.some(engine => removedModels.has(engine.modelId) || (engine.templateId !== null && removedTemplates.has(engine.templateId))) || (binding.templateId !== null && removedTemplates.has(binding.templateId));
        return { ...binding, enabled: affected ? false : binding.enabled, templateId: binding.templateId && removedTemplates.has(binding.templateId) ? null : binding.templateId, engines: binding.engines.filter(engine => !removedModels.has(engine.modelId)).map(engine => ({ ...engine, templateId: engine.templateId && removedTemplates.has(engine.templateId) ? null : engine.templateId })) };
      }),
    };
    onChange(next); onSelect(entity.kind === 'connection' ? next.connections[0]?.connectionId ?? null : next.models[0]?.modelId ?? null); setReview(null);
  };
  const name = entity.value.displayName;
  return <><Group title="引用关系"><FormRow label="草稿用途"><span>{purposes.length ? purposes.map(binding => `${binding.scopeKey === 'global' ? '全局' : snapshot.games.find(game => game.gameId === binding.gameId)?.displayName ?? '游戏'} · ${purposeNames[binding.purpose] ?? binding.purpose}`).join('；') : '尚未分配用途'}</span></FormRow><FormRow label="管理" note="仅编辑草稿；历史、系统凭据与已应用版本保留。"><Button intent="danger" label={entity.kind === 'connection' ? '从草稿移除连接…' : '从草稿移除模型配置…'} onPress={() => setReview({ configuration, entity })}>从草稿移除…</Button></FormRow></Group>
    <Sheet title={entity.kind === 'connection' ? '从草稿移除连接？' : '从草稿移除模型配置？'} description={name} open={review !== null} onClose={() => setReview(null)} footer={<><Button onPress={() => setReview(null)}>取消</Button><Button intent="danger" disabled={stale} onPress={remove}>从草稿移除</Button></>}>
      {stale ? <Notice error>草稿已变化，请取消后重新核对引用。</Notice> : null}
      <p>移除下列草稿对象及其专用模板，清除引擎引用，并停用受到影响的用途草稿。保存后才写入新草稿版本，不改写历史。</p>
      <ul className={styles.summary}>{configuration.models.filter(model => affectedModels.has(model.modelId)).map(model => <li key={model.modelId}>模型：{model.displayName}</li>)}{purposes.map(binding => <li key={binding.bindingId}>{binding.scopeKey === 'global' ? '全局' : '游戏专用'} · {purposeNames[binding.purpose] ?? binding.purpose}</li>)}</ul>
      {appliedReferences.length ? <Notice>已应用版本仍引用此对象；请在“测试与运行”中先停用，再检查并应用新草稿。此处不会原子删除或停用已应用版本。</Notice> : <Notice>这只修改编辑草稿，不删除阅读历史或系统凭据。</Notice>}
    </Sheet>
  </>;
}

export function CopyConnectionButton({ connection, configuration, onChange, onSelect }: { connection: Connection; configuration: Configuration; onChange: (value: Configuration) => void; onSelect: (id: string) => void }) {
  const [review, setReview] = useState<{ source: Connection; configuration: Configuration; name: string; reuseCredential: boolean } | null>(null);
  const stale = review !== null && review.configuration !== configuration;
  return <><Button label="复制服务连接…" onPress={() => setReview({ source: structuredClone(connection), configuration, name: `${connection.displayName} 副本`, reuseCredential: false })}>复制</Button><Sheet title="复制服务连接？" description="创建独立的连接草稿，不复制明文凭据，不修改原连接或当前运行配置。" open={review !== null} onClose={() => setReview(null)} footer={<><Button onPress={() => setReview(null)}>取消</Button><Button disabled={!review?.name.trim() || stale} intent="primary" onPress={() => {
    if (!review || stale) return;
    const connectionId = crypto.randomUUID();
    const copy: Connection = { ...structuredClone(review.source), connectionId, displayName: review.name.trim(), credentialPoolId: review.reuseCredential ? review.source.credentialPoolId : null, credentialPoolRevision: review.reuseCredential ? review.source.credentialPoolRevision : null };
    onChange({ ...configuration, connections: [...configuration.connections, copy] }); onSelect(connectionId); setReview(null);
  }}>创建连接副本</Button></>}>
    {stale ? <Notice error>草稿已变化，请取消后重新核对来源。</Notice> : null}
    <TextField label="连接副本名称" value={review?.name ?? ''} onChange={name => setReview(value => value ? { ...value, name } : value)}/>
    {review?.source.credentialPoolId ? <ConfigurationSwitch label="复用此连接的系统凭据引用" selected={review.reuseCredential} onChange={reuseCredential => setReview(value => value ? { ...value, reuseCredential } : value)} description="默认不复用。勾选后两个连接引用同一凭据池；不会复制或显示密钥明文。"/> : <p className={styles.help}>来源连接没有系统凭据引用。副本需要的认证可在编辑后单独配置。</p>}
  </Sheet></>;
}
