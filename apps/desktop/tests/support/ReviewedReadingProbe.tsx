/** Controlled UI data only. This module is excluded from the production entry. */
import { useState } from 'react';
import type { LexicalPresentationState, Preferences, PreviewProjection } from '../../src/bridge/contracts.generated';
import { Select } from '../../src/ui/primitives/Controls';
import { SourceText } from '../../src/features/language';
import { ReadingPanel } from '../../src/features/reading/ReadingPanel';
import { ApplicationStore, useApplication } from '../../src/bridge/projections/application-store';
import { WindowPreview } from '../../src/features/capture';
import type { ApplicationPort } from '../../src/bridge/application-port';
import { TauriApplicationPort } from '../../src/bridge/tauri-port';
import { emptySnapshot, fixtureUnit } from './design-port';

export function ReviewedReadingProbe() {
  const probe = new URLSearchParams(location.search).get('subscriptionProbe');
  return probe === 'reading-adapter' ? <PreviewReadingAdapterProbe/> : probe ? <PreviewSubscriptionProbe/> : <ReadingControlsProbe/>;
}
function PreviewReadingAdapterProbe() {
  const fixture = Reflect.get(window, 'uiHarness');
  const [fixtureStore] = useState(() => new ApplicationStore(fixture));
  const fixtureState = useApplication(fixtureStore);
  const [store] = useState(() => new ApplicationStore(new TauriApplicationPort()));
  return fixtureState.snapshot ? <ReadingPanel snapshot={fixtureState.snapshot} store={store} onConnect={() => {}} onStreams={() => {}}/> : null;
}
function PreviewSubscriptionProbe() {
  const [showA, setShowA] = useState(true);
  const [showB, setShowB] = useState(true);
  const fixture = Reflect.get(window, 'uiHarness');
  const mode = new URLSearchParams(location.search).get('subscriptionProbe');
  const candidate = mode === 'candidate-adapter';
  const [port] = useState<ApplicationPort>(() => mode === 'adapter' || candidate ? new TauriApplicationPort() : fixture);
  const [initialCandidate] = useState<PreviewProjection | null>(() => candidate ? structuredClone([...fixture.previewProjections.values()].at(-1) as PreviewProjection | undefined) ?? null : null);
  const props = { port, candidateId: candidate ? 'controlled-unresolved-candidate' : '', consumer: candidate ? 'candidate' as const : 'session' as const, consumerId: fixture.state.session.sessionId, candidateProjection: initialCandidate, detail: '受控订阅探针' };
  return <section data-testid="preview-subscription-probe"><button onClick={() => setShowA(value => !value)}>切换订阅 A</button><button onClick={() => setShowB(value => !value)}>切换订阅 B</button>{showA ? <div data-testid="preview-subscription-A"><WindowPreview {...props} title="A"/></div> : null}{showB ? <div data-testid="preview-subscription-B"><WindowPreview {...props} title="B"/></div> : null}</section>;
}
function ReadingControlsProbe() {
  const [value, setValue] = useState('body');
  const [failed, setFailed] = useState(true);
  const [preferences, setPreferences] = useState<Preferences>(emptySnapshot().preferences);
  const [invalid, setInvalid] = useState(false);
  const [unknownReading, setUnknownReading] = useState(false);
  const [clicked, setClicked] = useState<number | null>(null);
  const [lexical, setLexical] = useState<LexicalPresentationState>('ready');
  const [staleLexical, setStaleLexical] = useState(false);
  const [retries, setRetries] = useState(0);
  const unit = fixtureUnit();
  if (invalid) unit.tokens[0].byteEnd += 1;
  if (unknownReading) unit.tokens[0].reading = '';
  return <section style={{ padding: 24, maxWidth: 600 }} data-testid="reviewed-reading-probe">
    <Select label="示例文本来源" value={value} onChange={setValue} options={[
      { value: 'body', label: 'Artemis · 正文' }, { value: 'interface', label: 'Artemis · 界面文本' },
      { value: 'long', label: 'GameN7_Steam.exe · Steam · MAGES2 · 长文本来源名称完整显示' },
      { value: 'disabled', label: '已退出来源 · 不可选择', disabled: true },
      ...Array.from({ length: 40 }, (_, index) => ({ value: `source-${index}`, label: `完整会话来源 ${index + 1}` })),
    ]}/>
    <p data-testid="confirmed-select-value">{value}</p>
    <Select label="空模型列表" value="" onChange={setValue} options={[]} placeholder="暂无可选模型"/>
    <Select label="不可用模型" value="offline" onChange={setValue} options={[{ value: 'offline', label: '连接已停用' }]} disabled/>
    <Select label="正在获取模型" value="" onChange={setValue} options={[]} state="loading"/>
    <Select label="获取失败的模型" value="study" onChange={setValue} options={failed ? [] : [{ value: 'study', label: '日语阅读模型' }]} state={failed ? 'error' : 'ready'} onRetry={() => setFailed(false)}/>
    <fieldset><legend>同句显示偏好</legend>{(['segmentation', 'ruby', 'pos'] as const).map(name => <label key={name}><input type="checkbox" aria-label={name} checked={name === 'segmentation' ? preferences.segmentationEnabled : name === 'ruby' ? preferences.showRuby : preferences.showPos} onChange={event => setPreferences(current => name === 'segmentation' ? { ...current, segmentationEnabled: event.target.checked } : { ...current, [name === 'ruby' ? 'showRuby' : 'showPos']: event.target.checked })}/>{name}</label>)}<label><input type="checkbox" aria-label="invalid-spans" checked={invalid} onChange={event => setInvalid(event.target.checked)}/>invalid spans</label><label><input type="checkbox" aria-label="unknown-reading" checked={unknownReading} onChange={event => setUnknownReading(event.target.checked)}/>unknown reading</label></fieldset>
    <Select label="词法状态" value={lexical} onChange={value => setLexical(value as LexicalPresentationState)} options={(['ready', 'pending', 'resource_unavailable', 'worker_failed', 'alignment_invalid', 'reading_unavailable'] as const).map(value => ({ value, label: value }))}/>
    <label><input type="checkbox" aria-label="stale-lexical" checked={staleLexical} onChange={event => setStaleLexical(event.target.checked)}/>old lexical identity</label>
    <SourceText unit={unit} preferences={preferences} lexicalState={{ unitId: unit.unitId, textRevision: unit.textRevision, sourceHash: staleLexical ? 'b'.repeat(64) : unit.sourceHash, state: lexical, errorCode: null }} onRetry={() => { setRetries(value => value + 1); setLexical('pending'); }} onToken={index => setClicked(index)}/>
    <output data-testid="token-index">{clicked}</output>
    <output data-testid="lexical-retries">{retries}</output>
  </section>;
}
