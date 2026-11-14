import type { AppSnapshot } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { Button } from '../../ui/primitives/Controls';
import { EmptyState, FormRow, Group, Notice, PageHeader, StatusLabel } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';

const stageNames: Record<string, string> = { storage: '本地数据', capture: '游戏取文', language: '分词与词典', translation: '翻译服务', credentials: '系统凭据', configuration: '模型配置', bootstrap: '启动检查', subscription: '视图同步' };
export function DiagnosticsScreen({ snapshot, store }: { snapshot: AppSnapshot; store: ApplicationStore }) {
  const action = useAction();
  return <><PageHeader title="诊断" description="按阶段查看接入、处理与保存状态。默认不展示原文和认证值。" actions={<Button pending={action.pending} onPress={() => void action.run(() => store.refresh())}>重新检查状态</Button>}/><div className={styles.body}>{action.error ? <Notice error>{action.error}</Notice> : null}<Group title="当前运行"><FormRow label="档案"><code>{snapshot.profileId}</code></FormRow><FormRow label="视图版本"><span>rev {snapshot.revision} · generation {snapshot.profileGeneration}</span></FormRow><FormRow label="游戏取文"><StatusLabel>{snapshot.session?.state ?? '尚未接入'}</StatusLabel></FormRow><FormRow label="模型配置"><span>草稿 v{snapshot.configuration.draftHeadRevision} / 应用 v{snapshot.configuration.appliedHeadRevision}</span></FormRow></Group><Group title="阶段事件">{snapshot.diagnostics.length ? snapshot.diagnostics.map((item, index) => <article className={styles.event} key={`${item.code}:${index}`}><div><StatusLabel>{stageNames[item.stage] ?? item.stage}</StatusLabel><code>{item.code}</code></div><p>{item.message}</p><small>{item.recoverable ? '可按当前阶段复核并重试' : '请先处理此项，再继续对应流程'}</small></article>) : <EmptyState title="目前没有诊断事件" description="还没有错误事件不代表所有产品功能已经完成验证。" icon="activity"/>}</Group><Notice>更改模型、凭据或网络后，请在设置中对相应连接与用途重新测试；取文与译文分别核对。</Notice></div></>;
}
