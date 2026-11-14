import { useState } from 'react';
import type { AppSnapshot, ReadingUnit } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { Notice, PageHeader } from '../../ui/patterns/Patterns';
import { ReadingContent, manualTranslationFontPx } from '../reading';
import { ServiceSelector, serviceSelectionRefreshKey } from '../model-config';
import styles from './styles.module.css';

export interface ManualInputScreenProps { snapshot: AppSnapshot; store: ApplicationStore; active?: boolean }
type SubmittedReading = Pick<AppSnapshot, 'profileId' | 'serverInstanceId' | 'profileGeneration'> & { unitId: string };

// An unassociated Hook also has gameId:null. Only an explicit, coherent manual
// source summary can establish ownership; missing provenance is not manual.
function isManualReading(unit: ReadingUnit | null | undefined): unit is ReadingUnit {
  return Boolean(unit && unit.unitId && unit.sessionId && unit.textRevision > 0 && unit.sourceHash
    && unit.gameId === null && unit.sourceSummary?.sourceKind === 'manual'
    && unit.sourceSummary.gameId === null && unit.sourceSummary.sessionId === unit.sessionId);
}

/** Main-only editor; saved text and translation tasks stay owned by the store. */
export function ManualInputScreen({ snapshot, store, active = true }: ManualInputScreenProps) {
  const [text, setText] = useState('');
  const [language, setLanguage] = useState('ja');
  const [submitted, setSubmitted] = useState<SubmittedReading | null>(null);
  const action = useAction(), settingsAction = useAction();
  const sameOwner = submitted?.profileId === snapshot.profileId && submitted.serverInstanceId === snapshot.serverInstanceId && submitted.profileGeneration === snapshot.profileGeneration;
  const submittedUnit = sameOwner ? snapshot.history.units.find(item => item.unitId === submitted.unitId) ?? (snapshot.current?.unitId === submitted.unitId ? snapshot.current : null) : null;
  const unit = isManualReading(submittedUnit) ? submittedUnit : isManualReading(snapshot.current) ? snapshot.current : null;
  const targetLanguages = [...new Set(unit?.translations.map(translation => translation.targetLanguage).filter(Boolean) ?? [])];
  const configure = () => void settingsAction.run(() => store.port.openSettingsPage('translation'));
  return <>
    <PageHeader title="手动翻译" description="粘贴或输入文字，使用当前翻译用途；不要求连接游戏。" actions={<>
      <ServiceSelector appearance="manual" port={store.port} profileGeneration={snapshot.profileGeneration} scope={{ kind: 'global' }} purpose="translation" refreshKey={serviceSelectionRefreshKey(snapshot)} onConfigure={configure} onSelected={() => { void store.refresh(); }}/>
      <Button pending={settingsAction.pending} onPress={configure}>翻译设置…</Button>
    </>}/>
    <div className={styles.body} data-manual-body>
      {settingsAction.error ? <Notice error>{settingsAction.error}</Notice> : null}
      {action.error ? <Notice error>{action.error} 输入已保留。</Notice> : null}
      <div className={styles.heading}>
        <h2>原文</h2>
        <div className={styles.languages} data-manual-languages>
          <Select label="原文语言" hideLabel value={language} onChange={setLanguage}><option value="ja">日语</option><option value="en">英语</option><option value="zh-CN">中文</option></Select>
          <span>→ {targetLanguages.length ? targetLanguages.join(' / ') : '目标语言随翻译服务设置'}</span>
        </div>
      </div>
      <div className={styles.inputField}><TextField label="原文" multiline rows={6} value={text} onChange={setText} placeholder="输入或粘贴需要阅读的文本…"/></div>
      <div className={styles.actions} role="group" aria-label="手动输入操作">
        <Button label="翻译并阅读" intent="primary" pending={action.pending} disabled={!text.trim()} onPress={() => void action.run(async () => {
          const owner = { profileId: snapshot.profileId, serverInstanceId: snapshot.serverInstanceId, profileGeneration: snapshot.profileGeneration };
          const receipt = await store.command({ kind: 'manual_input', payload: { text, sourceLanguage: language } });
          if (receipt.resultId) setSubmitted({ ...owner, unitId: receipt.resultId });
        })}>翻译</Button>
        <Button label="清空输入" disabled={action.pending || !text} onPress={() => setText('')}>清空</Button>
        {action.pending ? <span role="status">正在保存原文…</span> : null}
      </div>
      <section className={styles.result} role="region" aria-label="手动翻译结果">
        {unit ? <ReadingContent snapshot={snapshot} store={store} unit={unit} active={active} translationFontPx={manualTranslationFontPx(snapshot.preferences)}/> : <p className={styles.placeholder}>译文将在这里显示。</p>}
      </section>
      <p className={styles.note}>使用已应用翻译配置；不会发送游戏的其他历史。</p>
    </div>
  </>;
}
