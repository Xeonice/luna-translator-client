import { ConfigurationRow as FormRow, ConfigurationSwitch as Switch } from './ConfigurationFields';
import { useRef, useState } from 'react';
import type { AppSnapshot, Connection } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { Group, Notice } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';
import { CredentialPoolEditor } from './CredentialPoolEditor';
import { CredentialRevocationPanel } from './CredentialRevocationPanel';
import type { PoolDraftCache } from './CredentialPoolEditor';

export function ConnectionEditor({ connection, onChange, store, snapshot, poolDraftCache, section = 'necessary' }: { connection: Connection; onChange: (value: Connection) => void; store: ApplicationStore; snapshot: AppSnapshot; poolDraftCache: PoolDraftCache; section?: 'necessary' | 'advanced' }) {
  const [secret, setSecret] = useState('');
  const [credentialLabel, setCredentialLabel] = useState('');
  const [maskedHint, setMaskedHint] = useState('');
  const action = useAction();
  const latest = useRef(connection); latest.current = connection;
  const patch = (value: Partial<Connection>) => onChange({ ...latest.current, ...value });
  if (section === 'advanced') return <>
    <Group title="连接与网络"><FormRow label="连接名称"><TextField label="连接名称" value={connection.displayName} onChange={displayName => patch({ displayName })}/></FormRow><FormRow label="协议"><Select label="服务协议" value={connection.protocol} onChange={protocol => patch({ protocol })}><option value="openai_compatible">OpenAI 兼容</option><option value="anthropic">Anthropic Messages</option><option value="azure_translator">Microsoft 文本翻译 v3</option></Select></FormRow><FormRow label="部署位置"><Select label="部署位置" value={connection.deployment} onChange={deployment => patch({ deployment })}><option value="cloud">云端</option><option value="local">本机 / 内网服务</option></Select></FormRow><FormRow label="代理"><Select label="代理策略" value={connection.proxyMode} onChange={proxyMode => patch({ proxyMode })}><option value="inherit">继承系统代理</option><option value="override">此连接独立代理</option><option value="disabled">直接连接</option></Select></FormRow>{connection.proxyMode === 'override' ? <FormRow label="独立代理地址"><TextField label="独立代理地址" type="url" value={typeof connection.networkMetadata.proxyUrl === 'string' ? connection.networkMetadata.proxyUrl : ''} onChange={proxyUrl => patch({ networkMetadata: { ...connection.networkMetadata, proxyUrl } })} placeholder="http://127.0.0.1:7890" description="填写完整协议、主机和端口。不要在 URL 中填写用户名、密码或密钥；保存草稿不代表已连通。"/></FormRow> : null}</Group>
    <Group title="连接可用性"><FormRow label="连接状态"><Switch label="此连接的草稿可用意图" selected={connection.availabilityIntent} onChange={availabilityIntent => patch({ availabilityIntent })} description="应用此意图影响新请求；保存或测试不会解除即时停用。"/></FormRow></Group>
    <div hidden={connection.authMode === 'none'}><CredentialPoolEditor snapshot={snapshot} connection={connection} store={store} onConnection={onChange} draftCache={poolDraftCache}/></div>
    {snapshot.credentials.length ? <CredentialRevocationPanel snapshot={snapshot} store={store}/> : null}
  </>;
  return <>
    <Group title="连接信息"><FormRow label="服务地址" note="按实际协议填写；本机无认证服务需要明确选择无认证。"><TextField label="服务地址" type="url" value={connection.endpoint ?? ''} onChange={endpoint => patch({ endpoint: endpoint || null })} placeholder="https://…"/></FormRow>{connection.protocol === 'azure_translator' ? <FormRow label="资源区域"><TextField label="Microsoft 资源区域" value={typeof connection.networkMetadata.region === 'string' ? connection.networkMetadata.region : ''} onChange={region => patch({ networkMetadata: { ...connection.networkMetadata, region } })}/></FormRow> : null}<FormRow label="认证方式"><Select label="认证方式" value={connection.authMode} onChange={authMode => patch({ authMode })}><option value="api_key">API Key</option><option value="none">无认证（需服务支持）</option></Select></FormRow></Group>
    <div hidden={connection.authMode === 'none'}><Group title="认证与系统密钥库" note="明文只在当前输入中短暂保留，保存后清空；配置与导出仅记录凭据引用。"><FormRow label="当前凭据"><span>{connection.credentialPoolId ? `${maskedHint || '已保存的系统凭据'} · pool v${connection.credentialPoolRevision ?? '?'}` : '尚未设置'}</span></FormRow><FormRow label="凭据名称"><TextField label="凭据名称" value={credentialLabel} onChange={setCredentialLabel} placeholder="仅供本机识别"/></FormRow><FormRow label="新 API Key"><TextField label="新 API Key" type="password" value={secret} onChange={setSecret} description="不会以明文回显；切换服务会保留当前输入，关闭窗口前请保存或主动清空。"/></FormRow><FormRow label="保存密钥"><div className={styles.poolActions}><Button disabled={!secret.trim() || !credentialLabel.trim()} pending={action.pending} onPress={() => void action.run(async () => { const credential = await store.port.storeCredential({ label: credentialLabel, secret }); setSecret(''); setMaskedHint(credential.maskedHint); patch({ credentialPoolId: credential.poolId, credentialPoolRevision: credential.poolRevision }); })}>保存到系统密钥库</Button><Button intent="quiet" disabled={!secret} onPress={() => setSecret('')}>清空密钥输入</Button></div></FormRow>{action.error ? <div className={styles.inlineError}><Notice error>{action.error} 密钥没有写入配置。</Notice></div> : null}</Group></div>
    {connection.authMode === 'none' ? <p className={styles.boundary}>无认证不会自动补上旧 Key；应用前请核对服务的访问范围。</p> : null}
  </>;
}
