import { ConfigurationRow as FormRow } from './ConfigurationFields';
import { useState } from 'react';
import type { AppSnapshot, CredentialRevocationImpact, CredentialRevocationRequest } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { Button, Select, Sheet } from '../../ui/primitives/Controls';
import { Group, Notice, StatusLabel } from '../../ui/patterns/Patterns';
import { AppliedPurposeImpact } from './RuntimeConfigurationPanel';
import styles from './styles.module.css';

type RevocationReview = { impact: CredentialRevocationImpact; request: CredentialRevocationRequest; attempted: boolean };

export function CredentialRevocationPanel({ snapshot, store }: { snapshot: AppSnapshot; store: ApplicationStore }) {
  const [selected, setSelected] = useState('');
  const [review, setReview] = useState<RevocationReview | null>(null);
  const action = useAction();
  const credential = snapshot.credentials.find(item => item.credentialId === selected) ?? snapshot.credentials[0];
  const reviewedCredential = snapshot.credentials.find(item => item.credentialId === review?.impact.credentialId);
  const stale = review !== null && !review.attempted && (review.impact.lifecycleRevision !== reviewedCredential?.lifecycleRevision || review.impact.configRevisionId !== (snapshot.configuration.applied?.configuration.revisionId ?? null));
  const preview = (credentialId: string) => action.run(async () => {
    const impact = await store.port.credentialRevocationImpact(credentialId);
    setReview({ impact, request: { operationId: crypto.randomUUID(), credentialId: impact.credentialId, expectedLifecycleRevision: impact.lifecycleRevision, expectedImpactHash: impact.impactHash }, attempted: false });
  });
  const revoke = () => {
    if (!review) return;
    const request = review.request;
    setReview(current => current ? { ...current, attempted: true } : current);
    void action.run(async () => {
      try { await store.port.revokeCredential(request); await store.refresh(); setReview(null); }
      catch (error) { await store.refresh(); throw error; }
    });
  };
  return <Group title="系统凭据 · 即时撤销" note="撤销会立即阻止使用该凭据的已应用连接，并取消相关请求。它独立于草稿、测试和应用；界面只读取脱敏引用。">
    {credential ? <>
      <FormRow label="系统凭据"><Select label="选择要撤销的凭据" value={credential.credentialId} onChange={setSelected}>{snapshot.credentials.map(item => <option key={item.credentialId} value={item.credentialId}>{item.maskedLabel}</option>)}</Select></FormRow>
      <FormRow label="当前状态"><StatusLabel>{credential.state === 'active' ? '当前有效' : credential.state === 'revoked' ? '已撤销' : '当前不可用'}</StatusLabel></FormRow>
      <FormRow label="安全操作"><Button intent="danger" pending={action.pending} disabled={credential.state === 'revoked'} onPress={() => void preview(credential.credentialId)}>检查撤销影响…</Button></FormRow>
    </> : <Notice>当前配置档还没有系统凭据引用。</Notice>}
    {action.error && !review ? <Notice error>{action.error} 尚未完成影响核对。</Notice> : null}
    <Sheet title="撤销这个系统凭据？" description="先按当前已应用版本核对共享引用，再明确撤销。相关连接会保持安全停用；保存、测试或应用草稿不会自行恢复。" open={review !== null} onClose={() => { if (!action.pending) { setReview(null); action.clearError(); } }} footer={<><Button disabled={action.pending} onPress={() => { setReview(null); action.clearError(); }}>取消</Button><Button intent="danger" pending={action.pending} disabled={stale} onPress={revoke}>{review?.attempted ? '重试本次撤销' : '确认撤销凭据'}</Button></>}>
      <p>{reviewedCredential?.maskedLabel ?? '已保存的系统凭据'}</p>
      {stale ? <Notice error>凭据或已应用引用已变化；请取消后重新检查影响。</Notice> : null}
      {action.error ? <Notice error>{action.error} 本次操作未完整完成，请以当前凭据和运行状态为准。重试沿用本次确认，不会创建新的撤销操作。</Notice> : null}
      <div className={styles.runtimeImpact}><strong>受影响的已应用连接</strong>{review?.impact.affectedConnectionIds.length ? <ul>{review.impact.affectedConnectionIds.map(id => <li key={id}>{snapshot.configuration.applied?.configuration.connections.find(connection => connection.connectionId === id)?.displayName ?? '已应用连接'}</li>)}</ul> : <p>当前已应用版本没有连接引用它。</p>}</div>
      <div className={styles.runtimeImpact}><strong>关联用途、引擎与游戏</strong>{review?.impact.affectedPurposes.length ? <ul>{review.impact.affectedPurposes.map(impact => <AppliedPurposeImpact key={impact.bindingId} snapshot={snapshot} impact={impact}/>)}</ul> : <p>当前没有关联的已应用用途。</p>}</div>
      <Notice>系统密钥库清理失败时，已完成的安全撤销不会被回滚。撤销不会删除阅读历史或其他凭据。</Notice>
    </Sheet>
  </Group>;
}
