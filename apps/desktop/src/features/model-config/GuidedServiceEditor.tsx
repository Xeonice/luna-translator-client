import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import type { CatalogPurpose, CatalogScalar, ServiceFormIssue, ServiceFormProjection, ServiceFormRequest, ServiceFormRow, ServiceFormTarget } from '../../bridge/contracts.generated';
import { Button, Select, Switch, TextField } from '../../ui/primitives/Controls';
import { guidedPurposeNames } from './ServiceCatalogPicker';
import styles from './guided.module.css';

export interface GuidedServiceFieldAction { label?: string; enabled: boolean; pending?: boolean; onPress: () => void }
export interface GuidedServiceEditorProps {
  resourceKey: string;
  actionsPlacement?: 'inline' | 'page-footer';
  title: string;
  subtitle?: string;
  projection: ServiceFormProjection | null;
  values: Readonly<Record<string, CatalogScalar>>;
  loading?: boolean;
  error?: string | null;
  busy?: boolean;
  canSave: boolean;
  canTest: boolean;
  testing?: boolean;
  cancellingTest?: boolean;
  onCancelTest?: () => void;
  result?: ReactNode;
  fieldControls?: Readonly<Record<string, ReactNode>>;
  fieldActions?: Readonly<Record<string, GuidedServiceFieldAction>>;
  onChange: (fieldId: string, value: CatalogScalar) => void;
  onSecretChange?: (fieldId: string, value: string) => void;
  onSave: () => void;
  onTest: () => void;
  onCancel: () => void;
  onChangeService?: () => void;
}

/** Transport races are handled here; conditions/defaults remain exclusively in Rust. */
function formIdentity(target: ServiceFormTarget, purpose: CatalogPurpose) {
  // Object key order is a transport detail. Rust/JSON maps may reorder it.
  return JSON.stringify(target.kind === 'http' ? ['http', target.preset.presetId, target.preset.presetRevision, purpose] : ['offline', target.engineId, target.contractRevision, purpose]);
}
export function useGuidedServiceForm(request: ServiceFormRequest | null, project: (request: ServiceFormRequest) => Promise<ServiceFormProjection>) {
  const key = request ? JSON.stringify(request) : null;
  const targetKey = request ? formIdentity(request.target, request.purpose) : null;
  const requestRef = useRef(request); requestRef.current = request;
  const sequence = useRef(0);
  const [state, setState] = useState<{ targetKey: string | null; projection: ServiceFormProjection | null; loading: boolean; error: string | null }>({ targetKey: null, projection: null, loading: false, error: null });
  useEffect(() => {
    const current = requestRef.current, generation = ++sequence.current;
    if (!current) { setState({ targetKey: null, projection: null, loading: false, error: null }); return; }
    setState(old => ({ targetKey, projection: old.targetKey === targetKey ? old.projection : null, loading: true, error: null }));
    void Promise.resolve().then(() => project(structuredClone(current))).then(projection => {
      if (formIdentity(projection.target, projection.purpose) !== targetKey) throw new Error('Service form response identity mismatch.');
      if (sequence.current === generation) setState({ targetKey, projection, loading: false, error: null });
    }).catch(() => {
      if (sequence.current === generation) setState(old => ({ ...old, loading: false, error: '无法更新服务字段，当前输入已保留。' }));
    });
    return () => { if (sequence.current === generation) sequence.current += 1; };
  }, [key, targetKey, project]);
  return state.targetKey === targetKey ? state : { targetKey, projection: null, loading: request !== null, error: null };
}

const issueLabels: Record<ServiceFormIssue, string> = {
  type_mismatch: '填写内容的类型不正确。', invalid_choice: '请选择当前接口提供的选项。', invalid_number: '数值超出允许范围。', invalid_url: '请填写有效的服务地址。', empty_value: '此处还没有填写内容。', required_value_missing: '请填写这项必要信息。', inactive_field: '该输入当前不适用，未用于有效配置。', condition_unresolved: '需要先确定相关配置。', dependency_cycle: '字段资料暂时无法解析。',
};
function fieldValue(row: ServiceFormRow, values: Readonly<Record<string, CatalogScalar>>) { return values[row.descriptor.id] ?? row.inputValue ?? row.resolved.value; }
function printable(value: CatalogScalar | null | undefined) { return value === null || value === undefined ? '' : String(value); }
function section(row: ServiceFormRow): 'necessary' | 'advanced' | 'details' | 'omit' {
  const field = row.descriptor;
  if (field.group === 'guidance' || field.group === 'internal' || field.classification === 'not_applicable') return 'omit';
  if (field.group === 'details' || field.classification === 'readonly') return 'details';
  if (field.group === 'advanced' || field.group === 'purpose' || field.classification === 'advanced' || field.classification === 'default') return 'advanced';
  return 'necessary';
}

/** Mount by stable resource identity: query updates preserve focus; changing resource erases secrets. */
export function GuidedServiceEditor(props: GuidedServiceEditorProps) { return <EditorBody key={props.resourceKey} {...props}/>; }
function EditorBody({ title, subtitle, actionsPlacement = 'inline', projection, values, loading = false, error, busy = false, canSave, canTest, testing = false, cancellingTest = false, onCancelTest, result, fieldControls = {}, fieldActions = {}, onChange, onSecretChange, onSave, onTest, onCancel, onChangeService }: GuidedServiceEditorProps) {
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const secretRef = useRef(secrets), secretCallback = useRef(onSecretChange);
  secretRef.current = secrets; secretCallback.current = onSecretChange;
  const clearSecrets = () => { for (const id of Object.keys(secretRef.current)) secretCallback.current?.(id, ''); secretRef.current = {}; setSecrets({}); };
  useEffect(() => () => { for (const id of Object.keys(secretRef.current)) secretCallback.current?.(id, ''); secretRef.current = {}; }, []);
  const visibleSecretIds = projection?.fields.filter(row => row.visibility === 'supported' && row.descriptor.fieldType === 'secret' && section(row) !== 'omit').map(row => row.descriptor.id) ?? [];
  const secretKey = JSON.stringify(visibleSecretIds);
  useEffect(() => {
    const hidden = Object.keys(secretRef.current).filter(id => !visibleSecretIds.includes(id));
    if (!hidden.length) return;
    const next = { ...secretRef.current }; for (const id of hidden) { delete next[id]; secretCallback.current?.(id, ''); }
    secretRef.current = next; setSecrets(next);
  }, [secretKey]);
  const changeSecret = (id: string, value: string) => { const next = { ...secretRef.current, [id]: value }; secretRef.current = next; setSecrets(next); onSecretChange?.(id, value); };
  const renderRow = (row: ServiceFormRow) => {
    const field = row.descriptor, value = fieldValue(row, values), action = fieldActions[field.id];
    if (row.visibility !== 'supported') return null;
    let control: ReactNode;
    const description = field.help ?? undefined;
    if (fieldControls[field.id] !== undefined) control = fieldControls[field.id];
    else if (field.fieldType === 'action') control = <div className={styles.actionField}><Button disabled={busy || !action?.enabled} pending={action?.pending} onPress={action?.onPress}>{action?.label ?? field.label}</Button>{description ? <small>{description}</small> : null}</div>;
    else if (field.fieldType === 'secret') control = <div className={styles.secretField}><TextField label={field.label} value={secrets[field.id] ?? ''} onChange={value => changeSecret(field.id, value)} type="password" required={row.required} disabled={busy || !onSecretChange || field.observedOnly || field.classification === 'readonly'} description={description}/>{action ? <Button disabled={busy || !action.enabled} pending={action.pending} onPress={action.onPress}>{action.label ?? '选择凭据文件…'}</Button> : null}</div>;
    else if (!row.inputAllowed || field.fieldType === 'status') control = <div className={styles.observation}><strong>{field.label}</strong><span>{printable(row.resolved.value) || (row.resolved.source === 'inherited' ? '继承当前设置' : '未提供')}</span>{description ? <small>{description}</small> : null}</div>;
    else if (field.fieldType === 'select') control = <Select label={row.required ? `${field.label}（必填）` : field.label} value={value === null || value === undefined ? '' : JSON.stringify(value)} disabled={busy} onChange={selected => { const choice = field.choices.find(choice => JSON.stringify(choice.value) === selected); if (choice) onChange(field.id, choice.value); }} options={field.choices.map(choice => ({ value: JSON.stringify(choice.value), label: choice.label }))}/>;
    else if (field.fieldType === 'boolean') control = <Switch label={field.label} selected={value === true} onChange={selected => onChange(field.id, selected)} disabled={busy} description={description}/>;
    else if (field.fieldType === 'path') control = <div className={styles.pathField}><TextField label={field.label} value={printable(value)} onChange={value => onChange(field.id, value)} disabled={busy} description={description}/><Button disabled={busy || !action?.enabled} pending={action?.pending} onPress={action?.onPress}>{action?.label ?? '浏览…'}</Button></div>;
    else control = <TextField label={field.label} value={printable(value)} onChange={value => onChange(field.id, field.fieldType === 'number' && value !== '' && Number.isFinite(Number(value)) ? Number(value) : value)} disabled={busy} required={row.required} type={field.fieldType === 'url' ? 'url' : field.fieldType === 'number' ? 'number' : 'text'} multiline={field.fieldType === 'textarea'} description={description}/>;
    return <div className={styles.field} data-service-field={field.id} data-field-visibility={row.visibility} data-field-classification={field.classification} key={field.id}>{control}{row.resolved.source === 'inherited' && row.inputAllowed ? <small className={styles.note}>继承{row.resolved.inheritFrom === 'source_language' ? '原文语言' : '当前目标语言'}</small> : null}{row.issues.length ? <div className={styles.fieldError} role="status">{row.issues.map(issue => <div key={issue}>{issueLabels[issue]}</div>)}</div> : null}</div>;
  };
  const rows = projection?.fields ?? [];
  const renderSection = (type: 'necessary' | 'advanced' | 'details') => rows.filter(row => section(row) === type).map(renderRow);
  const advanced = rows.some(row => section(row) === 'advanced' && row.visibility === 'supported');
  const details = rows.some(row => section(row) === 'details' && row.visibility === 'supported');
  const unresolved = rows.some(row => section(row) === 'necessary' && row.visibility === 'unknown');
  return <section className={styles.editor} data-guided-service-editor aria-label="服务配置编辑" aria-busy={loading || busy}>
    <header><h3>{title}</h3>{subtitle ? <p>{subtitle}</p> : null}</header>
    {projection ? <div className={styles.editorSummary}><span>用于{guidedPurposeNames[projection.purpose]}</span>{onChangeService ? <Button intent="quiet" disabled={busy} onPress={onChangeService}>更换服务…</Button> : null}</div> : null}
    {loading ? <p className={styles.note} role="status">正在更新服务字段…</p> : null}
    {error ? <p className={styles.fieldError} role="alert">{error}</p> : null}
    <div className={styles.fields}>{renderSection('necessary')}</div>
    {unresolved ? <p className={styles.note} role="status">相关选项会在前面的配置确定后显示。</p> : null}
    {projection?.materialBlockers.filter(item => item.conditionState !== 'unsupported').map((item, index) => <p className={styles.fieldError} role="status" key={index}>{item.conditionState === 'unknown' ? '待确定：' : ''}{item.descriptor.message}</p>)}
    {advanced ? <details className={styles.disclosure}><summary>高级设置</summary><div className={styles.fields}>{renderSection('advanced')}</div></details> : null}
    {details ? <details className={styles.disclosure}><summary>接口与配置说明</summary><div className={styles.fields}>{renderSection('details')}</div></details> : null}
    <div className={styles.actions}>{testing ? <Button disabled={cancellingTest || !onCancelTest} onPress={onCancelTest}>{cancellingTest ? '正在取消…' : '取消测试'}</Button> : <Button disabled={!canTest || busy || loading} onPress={onTest}>翻译一句试试</Button>}{actionsPlacement === 'inline' ? <><Button intent="primary" disabled={!canSave || busy || loading} pending={busy} onPress={onSave}>保存配置</Button><Button disabled={busy} onPress={() => { clearSecrets(); onCancel(); }}>取消</Button></> : null}<small>保存不切换当前服务；测试结果仅属于这项配置。</small></div>
    {result ? <div className={styles.testResult} role="status">{result}</div> : null}
  </section>;
}
