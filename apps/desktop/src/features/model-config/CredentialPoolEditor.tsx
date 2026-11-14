import { ConfigurationRow as FormRow, ConfigurationSwitch as Switch } from './ConfigurationFields';
import { useEffect, useRef, useState } from 'react';
import type { AppSnapshot, Connection, CredentialPool, CredentialPoolInput } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { Group, Notice, StatusLabel } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';

function poolInput(pool: CredentialPool | undefined): CredentialPoolInput { return pool ? { poolId: pool.poolId, expectedRevision: pool.revision, policy: pool.policy, metadata: { ...pool.metadata }, members: pool.members.map(member => ({ memberId: member.memberId, credentialId: member.credentialId, enabled: member.enabled, weight: member.weight })) } : { poolId: null, expectedRevision: 0, policy: 'selected', metadata: {}, members: [] }; }
export type PoolDraftCache = Map<string, { selected: string; draft: CredentialPoolInput; dirty: boolean }>;
function poolRef(pool: CredentialPool) { return `${pool.poolId}@${pool.revision}`; }
export function CredentialPoolEditor({ snapshot, connection, store, onConnection, draftCache }: { snapshot: AppSnapshot; connection: Connection; store: ApplicationStore; onConnection: (connection: Connection) => void; draftCache: PoolDraftCache }) {
  const initial = snapshot.credentialPools.find(pool => pool.poolId === connection.credentialPoolId && pool.revision === connection.credentialPoolRevision);
  const cached = draftCache.get(connection.connectionId);
  const [selected, setSelected] = useState(cached?.selected ?? (initial ? poolRef(initial) : ''));
  const [draft, setDraft] = useState<CredentialPoolInput>(() => cached?.draft ?? poolInput(initial));
  const [dirty, setDirty] = useState(cached?.dirty ?? false);
  const boundRef = useRef(initial ? poolRef(initial) : '');
  useEffect(() => { draftCache.set(connection.connectionId, { selected, draft, dirty }); }, [draftCache, connection.connectionId, selected, draft, dirty]);
  useEffect(() => { const bound = snapshot.credentialPools.find(pool => pool.poolId === connection.credentialPoolId && pool.revision === connection.credentialPoolRevision); const ref = bound ? poolRef(bound) : ''; if (ref !== boundRef.current && !dirty) { boundRef.current = ref; setSelected(ref); setDraft(poolInput(bound)); } }, [snapshot.credentialPools, connection.credentialPoolId, connection.credentialPoolRevision, dirty]);
  const action = useAction();
  const patch = (value: Partial<CredentialPoolInput>) => { setDraft(current => ({ ...current, ...value })); setDirty(true); };
  const activeCredentials = snapshot.credentials.filter(credential => credential.state === 'active');
  const currentPool = snapshot.credentialPools.find(pool => poolRef(pool) === selected);
  const select = (value: string) => { const pool = snapshot.credentialPools.find(pool => poolRef(pool) === value); setSelected(value); setDraft(poolInput(pool)); setDirty(false); };
  const memberEnabled = (credentialId: string, enabled: boolean) => {
    const exists = draft.members.some(member => member.credentialId === credentialId);
    const members = draft.members.map(member => member.credentialId === credentialId ? { ...member, enabled } : draft.policy === 'selected' && enabled ? { ...member, enabled: false } : member);
    if (!exists) members.push({ memberId: null, credentialId, enabled, weight: 1 }); patch({ members });
  };
  return <Group title="密钥池与轮换" note="新池版本不会自动替换连接使用的旧版本；需明确选择，再保存并应用连接配置。"><FormRow label="已有池版本"><Select label="选择密钥池版本" value={selected} onChange={select}><option value="">新建密钥池</option>{snapshot.credentialPools.map(pool => <option key={poolRef(pool)} value={poolRef(pool)}>{typeof pool.metadata.label === 'string' ? pool.metadata.label : '密钥池'} · v{pool.revision} · {pool.policy}</option>)}</Select></FormRow><FormRow label="池名称"><TextField label="池名称" value={typeof draft.metadata.label === 'string' ? draft.metadata.label : ''} onChange={label => patch({ metadata: { ...draft.metadata, label } })}/></FormRow><FormRow label="选择策略"><Select label="密钥选择策略" value={draft.policy} onChange={policy => patch({ policy })}><option value="selected">固定选用一个凭据</option><option value="round_robin">依次轮换</option><option value="weighted">按权重轮换</option></Select></FormRow>{activeCredentials.length ? activeCredentials.map(credential => { const member = draft.members.find(member => member.credentialId === credential.credentialId); return <FormRow key={credential.credentialId} label={credential.maskedLabel} note="仅显示系统凭据引用"><div className={styles.poolMember}>{draft.policy === 'selected' ? <label><input type="radio" name={`credential-${connection.connectionId}`} checked={member?.enabled ?? false} onChange={() => memberEnabled(credential.credentialId, true)}/> 选用此凭据</label> : <Switch label="加入此池" selected={member?.enabled ?? false} onChange={enabled => memberEnabled(credential.credentialId, enabled)}/>} {draft.policy === 'weighted' && member ? <TextField label={`${credential.maskedLabel}权重`} type="number" value={String(member.weight)} onChange={value => { const weight = Number(value); if (Number.isInteger(weight) && weight >= 0) patch({ members: draft.members.map(item => item.credentialId === credential.credentialId ? { ...item, weight } : item) }); }}/> : null}</div></FormRow>; }) : <Notice>先在上方保存系统凭据，再建立池。不会显示或导出明文。</Notice>}{action.error ? <Notice error>{action.error} 池编辑保持不变。</Notice> : null}<FormRow label="版本操作"><div className={styles.poolActions}><Button pending={action.pending} disabled={!dirty || !draft.members.length} onPress={() => void action.run(async () => { const receipt = await store.command({ kind: 'save_credential_pool', payload: { pool: draft } }); const saved = store.getSnapshot().snapshot?.credentialPools.find(pool => poolRef(pool) === receipt.resultId); if (saved) { setSelected(poolRef(saved)); setDraft(poolInput(saved)); setDirty(false); } })}>保存新的池版本</Button><Button intent="primary" disabled={dirty || !currentPool} onPress={() => { if (currentPool) onConnection({ ...connection, credentialPoolId: currentPool.poolId, credentialPoolRevision: currentPool.revision }); }}>用于此连接草稿</Button>{currentPool ? <StatusLabel>v{currentPool.revision} · 不自动应用</StatusLabel> : null}</div></FormRow></Group>;
}
