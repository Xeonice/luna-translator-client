import { ConfigurationRow as FormRow, ConfigurationSwitch as Switch } from './ConfigurationFields';
import type { ReactNode } from 'react';
import type { Configuration, PurposeBinding, PurposeRuntimeView, Template } from '../../bridge/contracts.generated';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { Group } from '../../ui/patterns/Patterns';
import { ParameterEditor } from './ParameterEditor';
import { scheduleLimits } from './draft-issues';
import styles from './styles.module.css';
import { purposeRuntimeStatus } from './runtime-status';

export const purposeNames: Record<string, string> = { translation: '实时翻译', grammar: '句子精读', context_word: '语境查词' };
export function PurposeEditor({ binding, configuration, onChange, onTemplate, sessionControls, section = 'all', applied = null, appliedRevision = 0, runtimePurposes = [] }: { binding: PurposeBinding; configuration: Configuration; onChange: (value: PurposeBinding) => void; onTemplate: (value: Template) => void; sessionControls?: ReactNode; section?: 'all' | 'parameters' | 'runtime'; applied?: Configuration | null; appliedRevision?: number; runtimePurposes?: PurposeRuntimeView[] }) {
  const patch = (value: Partial<PurposeBinding>) => onChange({ ...binding, ...value });
  const template = configuration.templates.find(item => item.templateId === binding.templateId);
  const engine = binding.engines[0];
  const model = configuration.models.find(item => item.modelId === engine?.modelId);
  const connection = configuration.connections.find(item => item.connectionId === model?.connectionId);
  const protocolName = connection ? ({ openai_compatible: 'OpenAI 兼容', anthropic: 'Anthropic Messages', azure_translator: 'Microsoft 文本翻译' } as Record<string, string>)[connection.protocol] ?? connection.protocol : null;
  const modelDescription = connection ? `${connection.displayName} · ${protocolName}；${connection.protocol === 'azure_translator' ? '传统翻译渠道，无模型 ID。' : `请求模型 ${model?.actualModelId ?? '尚未填写'}。`}` : '请选择模型配置。';
  const appliedBinding = applied?.bindings.find(item => item.scopeKey === binding.scopeKey && item.purpose === binding.purpose) ?? applied?.bindings.find(item => item.scopeKey === 'global' && item.purpose === binding.purpose);
  const appliedNames = appliedBinding?.engines.filter(item => item.enabled).map(item => applied?.models.find(model => model.modelId === item.modelId)?.displayName).filter(Boolean) ?? [];
  const runtime = runtimePurposes.find(item => item.scopeKey === appliedBinding?.scopeKey && item.purpose === binding.purpose && item.configRevisionId === applied?.revisionId);
  const setPrimary = (modelId: string) => patch({ engines: modelId ? [{ engineId: engine?.engineId ?? crypto.randomUUID(), modelId, enabled: engine?.enabled ?? true, templateId: engine?.templateId ?? null, parameters: engine?.parameters ?? {} }, ...binding.engines.slice(1)] : [] });
  const updateTemplate = (text: string) => {
    const next: Template = template ? { ...template, body: { ...template.body, system: text } } : { templateId: crypto.randomUUID(), purpose: binding.purpose, displayName: `${purposeNames[binding.purpose]}模板`, sourceKind: 'custom', sourceVersion: null, body: { system: text }, variables: ['source', 'history', 'terms'], compatibility: {} };
    onTemplate(next);
    if (!template) patch({ templateId: next.templateId });
  };
  const moveEngine = (index: number, offset: number) => {
    const destination = index + offset;
    if (destination < 0 || destination >= binding.engines.length) return;
    const engines = [...binding.engines];
    [engines[index], engines[destination]] = [engines[destination]!, engines[index]!];
    patch({ engines });
  };
  return <>
    {section === 'all' ? <><Group title="用途与来源">
      <FormRow label="使用配置" note={modelDescription}><Select label="主模型配置" value={engine?.modelId ?? ''} onChange={setPrimary}><option value="">选择模型配置…</option>{configuration.models.map(model => <option key={model.modelId} value={model.modelId}>{model.displayName}</option>)}</Select></FormRow>
      <FormRow label="用途状态"><Switch label={`${purposeNames[binding.purpose] ?? binding.purpose}的草稿启用意图`} selected={binding.enabled} onChange={enabled => patch({ enabled })} description="应用后才采用此意图；保存或测试不会恢复已停用的运行许可。"/></FormRow>
      <FormRow label={binding.purpose === 'translation' ? '翻译语言' : '解释语言'}><Select label="目标语言" value={binding.targetLanguage ?? 'zh-CN'} onChange={targetLanguage => patch({ targetLanguage })}><option value="zh-CN">简体中文</option><option value="en">English</option><option value="ja">日本語</option></Select></FormRow>
      {binding.purpose !== 'translation' ? <FormRow label="输出模式"><Select label="精读输出模式" value={binding.outputMode ?? 'plain_text'} onChange={outputMode => patch({ outputMode })}><option value="plain_text">文字解释</option><option value="structured">结构化解释（需校验引用）</option></Select></FormRow> : null}
      {sessionControls}
    </Group>
    <Group title="输入范围">
      <FormRow label="近期上下文" note="0 表示只发送当前句。按同一游戏、同一会话隔离。"><div className={styles.inlineNumber}><TextField label="上下文句数" type="number" value={String(binding.context.maxSentences ?? 0)} onChange={value => { const count = Number(value); if (Number.isInteger(count) && count >= 0) patch({ context: { ...binding.context, maxSentences: count, enabled: count > 0 } }); }}/></div><span className={styles.inlineUnit}>句</span></FormRow>
      <FormRow label="历史内容"><Select label="上下文内容" value={String(binding.context.historyMode ?? 'original')} onChange={historyMode => patch({ context: { ...binding.context, historyMode } })}><option value="original">仅原文</option><option value="translation">仅译文</option><option value="both">原文与译文</option></Select></FormRow>
      <FormRow label="有效配置" note="已应用配置与当前运行许可分别保留。"><span className={styles.effectiveSummary}>{appliedNames.length ? `${appliedNames.join('、')} · v${appliedRevision}` : '尚未配置模型'}</span><span className={styles.badge}>{!appliedBinding ? '尚未应用' : purposeRuntimeStatus(runtime)}</span></FormRow>
    </Group>
    </> : null}
    {section === 'runtime' ? <Group title="请求调度" note="各用途分别排队。并列引擎共享此用途的总并发；修改并应用后用于新请求。">
      <FormRow label="请求超时（秒）"><TextField label="请求超时秒数" type="number" value={String(Number(binding.schedule.timeoutMs ?? 60000) / 1000)} onChange={value => { const seconds = Number(value); if (Number.isFinite(seconds)) patch({ schedule: { ...binding.schedule, timeoutMs: seconds * 1000 } }); }}/></FormRow>
      {Object.entries(scheduleLimits).map(([key, limit]) => <FormRow key={key} label={limit.label} note={key === 'retries' ? '0 表示不自动重试；最多额外重试 3 次，不会切换服务。' : key === 'concurrency' ? '1–8 个在途请求；默认 1。' : '1–128 个待执行请求；默认 32。队列满时保留明确错误。'}><TextField label={limit.label} type="number" value={binding.schedule[key] === null ? '' : String(binding.schedule[key] ?? limit.fallback)} onChange={value => patch({ schedule: { ...binding.schedule, [key]: value.trim() === '' ? null : Number(value) } })}/></FormRow>)}
    </Group>
    : null}
    {section === 'all' && binding.purpose === 'translation' ? <Group title="并列翻译引擎" note="按列表顺序展示，各引擎独立显示结果与失败。开关和排序先保存为草稿，应用后才生效。">
      {binding.engines.map((item, index) => <FormRow label={`引擎${index + 1}`} key={item.engineId}>
        <div className={styles.engine}><Select label={`引擎 ${index + 1} 模型`} value={item.modelId} onChange={modelId => patch({ engines: binding.engines.map(value => value.engineId === item.engineId ? { ...value, modelId } : value) })}>{configuration.models.map(model => <option key={model.modelId} value={model.modelId}>{model.displayName}</option>)}</Select><Switch label={`启用引擎${index + 1}`} selected={item.enabled} onChange={enabled => patch({ engines: binding.engines.map(value => value.engineId === item.engineId ? { ...value, enabled } : value) })}/></div>
        <div className={styles.engineActions}><Button intent="quiet" disabled={index === 0} onPress={() => moveEngine(index, -1)} label={`上移引擎 ${index + 1}`}>上移</Button><Button intent="quiet" disabled={index === binding.engines.length - 1} onPress={() => moveEngine(index, 1)} label={`下移引擎 ${index + 1}`}>下移</Button><Button intent="quiet" label={`移除引擎 ${index + 1}`} onPress={() => patch({ engines: binding.engines.filter(value => value.engineId !== item.engineId) })}>从草稿移除</Button></div>
      </FormRow>)}
      <FormRow label="附加引擎"><div className={styles.addEngine}><Button disabled={!configuration.models.length} onPress={() => patch({ engines: [...binding.engines, { engineId: crypto.randomUUID(), modelId: configuration.models[0]?.modelId ?? '', enabled: true, templateId: null, parameters: {} }] })}>添加并列引擎</Button></div></FormRow>
    </Group> : null}
    {section === 'parameters' ? <><Group title="用途参数覆盖"><ParameterEditor parameters={binding.parameters} onChange={parameters => patch({ parameters })}/></Group>
    <Group title="提示词模板"><FormRow label="模板操作"><Button intent="quiet" disabled={!binding.templateId} onPress={() => patch({ templateId: null })}>恢复模板继承</Button></FormRow><FormRow label="模板内容"><TextField label="用途模板" value={typeof template?.body.system === 'string' ? template.body.system : ''} onChange={updateTemplate} multiline description="可用变量 {{source}}、{{history}}、{{terms}}；新模板在草稿中保存，应用前不会发请求。"/></FormRow></Group>
    </> : null}
    {section === 'all' ? <p className={styles.boundary}>内置分词与词典始终可用。精读和语境查词只在你主动发起后使用模型。</p> : null}
  </>;
}
