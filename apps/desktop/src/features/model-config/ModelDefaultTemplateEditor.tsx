import { ConfigurationRow as FormRow } from './ConfigurationFields';
import type { Configuration, Model, Template } from '../../bridge/contracts.generated';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { Group, Notice } from '../../ui/patterns/Patterns';
import { purposeNames } from './PurposeEditor';

const purposes = ['translation', 'grammar', 'context_word'];
export function ModelDefaultTemplateEditor({ model, configuration, onChange, onTemplate, supportsTemplates = true }: { model: Model; configuration: Configuration; onChange: (model: Model) => void; onTemplate: (template: Template) => void; supportsTemplates?: boolean }) {
  const defaults = model.defaultTemplates ?? {};
  const select = (purpose: string, templateId: string) => { const next = { ...defaults }; if (templateId) next[purpose] = templateId; else delete next[purpose]; onChange({ ...model, defaultTemplates: next }); };
  const create = (purpose: string) => {
    const label = `${purposeNames[purpose]}专用模板`;
    const names = new Set(configuration.templates.filter(t => t.purpose === purpose && t.compatibility.modelConfigurationId === model.modelId).map(t => t.displayName));
    let name = label; let n = 2; while (names.has(name)) name = `${label} ${n++}`;
    const value: Template = { templateId: crypto.randomUUID(), purpose, displayName: name, sourceKind: 'custom', sourceVersion: null, body: { user: '{{source}}' }, variables: ['source'], compatibility: { modelConfigurationId: model.modelId } };
    onTemplate(value); select(purpose, value.templateId);
  };
  return <Group title="模型默认模板" note="按用途分别保存。实际优先采用引擎和用途覆盖，再使用模型默认；未选择时采用内置模板。保存和恢复只修改草稿，不会发起请求。">
    {!supportsTemplates ? <Notice>此传统翻译协议不支持提示词模板；请恢复内置处理，已有草稿可清除后保存。</Notice> : null}
    {purposes.map(purpose => {
      const template = configuration.templates.find(t => t.templateId === defaults[purpose]);
      const owned = template?.compatibility.modelConfigurationId === model.modelId;
      return <section key={purpose} aria-label={`${purposeNames[purpose]}模型默认模板`}>
        <FormRow label={purposeNames[purpose]}><Select disabled={!supportsTemplates} label={`${purposeNames[purpose]}默认模板`} value={defaults[purpose] ?? ''} onChange={id => select(purpose, id)}><option value="">继承内置用途模板</option>{configuration.templates.filter(t => t.purpose === purpose && (t.compatibility.modelConfigurationId === undefined || t.compatibility.modelConfigurationId === model.modelId)).map(t => <option key={t.templateId} value={t.templateId}>{t.displayName} · {t.compatibility.modelConfigurationId ? '此模型专用' : '共享引用'}</option>)}</Select></FormRow>
        <FormRow label="模板操作"><Button disabled={!supportsTemplates} intent="quiet" onPress={() => create(purpose)}>新建此模型专用模板</Button><Button intent="quiet" disabled={!defaults[purpose]} onPress={() => select(purpose, '')}>恢复内置用途模板</Button></FormRow>
        {owned && template ? <><FormRow label="模板名称"><TextField disabled={!supportsTemplates} label={`${purposeNames[purpose]}专用模板名称`} value={template.displayName} onChange={displayName => onTemplate({ ...template, displayName })}/></FormRow><FormRow label="System"><TextField disabled={!supportsTemplates} multiline label={`${purposeNames[purpose]}专用system模板`} value={typeof template.body.system === 'string' ? template.body.system : ''} onChange={system => { const body = { ...template.body }; if (system.trim()) body.system = system; else delete body.system; onTemplate({ ...template, body }); }} description="留空时继承内置用途说明"/></FormRow><FormRow label="User"><TextField disabled={!supportsTemplates} multiline label={`${purposeNames[purpose]}专用user模板`} value={typeof template.body.user === 'string' ? template.body.user : ''} onChange={user => onTemplate({ ...template, body: { ...template.body, user } })}/></FormRow></> : template ? <Notice>这是共享模板引用。复制模型会继续引用它；需要独立内容时新建此模型专用模板，不修改共享原件。</Notice> : null}
      </section>;
    })}
  </Group>;
}
