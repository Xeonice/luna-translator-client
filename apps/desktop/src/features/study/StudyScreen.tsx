import { useEffect, useId, useState } from 'react';
import type { AnalysisReference, AppSnapshot, StudyResult, WordLookup } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { useAction } from '../../bridge/projections';
import { Button } from '../../ui/primitives/Controls';
import { Icon } from '../../ui/icons/Icon';
import { EmptyState, Notice, StatusLabel } from '../../ui/patterns/Patterns';
import { SourceText, DictionaryEntries } from '../language';
import { TranslationStack, readingOriginLabel } from '../reading';
import styles from './styles.module.css';

type StudyTab = 'grammar' | 'dictionary';
function StudyExplanation({ result, onReference }: { result: StudyResult; onReference: (reference: AnalysisReference) => void }) {
  return <article className={styles.explanation}><div className={styles.tags}><StatusLabel>AI 解释 · {result.mode === 'structured' ? '引用已校验' : '文字模式'}</StatusLabel><StatusLabel>{result.purpose === 'grammar' ? '本句文法分析' : '语境查词'}</StatusLabel></div><p>{result.text}</p>{result.mode === 'structured' && result.references.length ? <div className={styles.references}>{result.references.map((reference, index) => <section key={index}><Button intent="quiet" onPress={() => onReference(reference)}>{reference.quote}</Button><p>{reference.explanation}</p></section>)}</div> : null}<small>AI 解释可能存在歧义，请结合原文与本地词典校对。</small></article>;
}
export function StudyScreen({ snapshot, store }: { snapshot: AppSnapshot; store: ApplicationStore }) {
  const frozen = snapshot.learningObject;
  const unit = frozen?.unit;
  const index = frozen?.tokenIndex;
  const [lookup, setLookup] = useState<WordLookup | null>(null);
  const [lookupError, setLookupError] = useState<string | null>(null);
  const [highlight, setHighlight] = useState<AnalysisReference | null>(null);
  const [tab, setTab] = useState<StudyTab>(index === null || index === undefined ? 'grammar' : 'dictionary');
  const tabId = useId();
  const action = useAction();
  useEffect(() => {
    setTab(index === null || index === undefined ? 'grammar' : 'dictionary');
    setHighlight(null); setLookup(null); setLookupError(null);
    if (!unit || index === null || index === undefined) return;
    let active = true;
    void store.port.lookup(unit.unitId, unit.textRevision, index).then(value => { if (active) setLookup(value); }).catch(value => { if (active) setLookupError(errorMessage(value)); });
    return () => { active = false; };
  }, [store, unit?.unitId, unit?.textRevision, index]);
  if (!unit) return <EmptyState title="先固定一句原文" description="从浮窗、当前会话或历史记录选择“精读本句”，或点击一个日语词语打开学习窗口。" icon="book"/>;
  const currentDiffers = snapshot.current && (snapshot.current.unitId !== unit.unitId || snapshot.current.textRevision !== unit.textRevision);
  const tasks = snapshot.tasks.filter(task => task.unitId === unit.unitId && task.textRevision === unit.textRevision && task.purpose !== 'translation');
  const pending = tasks.filter(task => task.status === 'queued' || task.status === 'running');
  const results = snapshot.studyResults.filter(result => result.unitId === unit.unitId && result.textRevision === unit.textRevision);
  const runtimePurposes = snapshot.configuration.runtimePurposes.filter(purpose => purpose.purpose === 'grammar' && (unit.gameId ? purpose.affectedGameIds.includes(unit.gameId) : purpose.scopeKey === 'global'));
  const resultProvider = results.length ? tasks.find(task => task.taskId === results.at(-1)?.taskId)?.providerLabel : null;
  const configured = runtimePurposes.some(purpose => purpose.enabledIntent && purpose.runtimeEnabled && purpose.authorizedEngineIds.length > 0 && purpose.blockedConnectionIds.length === 0);
  const request = (purpose: string) => action.run(async () => { setTab('grammar'); await store.port.study(unit.unitId, unit.textRevision, purpose, frozen.tokenIndex); await store.refresh(); });
  const openSettings = () => void action.run(() => store.port.openWindow('settings'));
  return <section className={styles.layout} aria-label="固定原句学习">
    <header className={styles.toolbar}><div className={styles.tabs} role="tablist" aria-label="学习内容">{(['grammar', 'dictionary'] as const).map(value => <button key={value} id={`${tabId}-${value}`} role="tab" aria-selected={tab === value} aria-controls={`${tabId}-panel`} tabIndex={tab === value ? 0 : -1} onClick={() => setTab(value)} onKeyDown={event => { if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) { event.preventDefault(); const next = event.key === 'Home' ? 'grammar' : event.key === 'End' ? 'dictionary' : value === 'grammar' ? 'dictionary' : 'grammar'; setTab(next); document.getElementById(`${tabId}-${next}`)?.focus(); } }}>{value === 'grammar' ? '本句精读' : '内置词典'}</button>)}</div><Button intent="quiet" onPress={openSettings}>模型设置…</Button></header>
    <div className={styles.document} data-ui-scroll="learning">
      <div className={styles.sourceHeading}><div><h1>正在理解的原文</h1><small>{unit.gameId ? (snapshot.games.find(game => game.gameId === unit.gameId)?.originalTitle || snapshot.games.find(game => game.gameId === unit.gameId)?.displayName) ?? '历史游戏' : readingOriginLabel(unit)}</small></div><small>已固定当前句</small></div>
      <SourceText unit={unit} preferences={snapshot.preferences} lexicalState={snapshot.lexicalStates.find(state => state.unitId === unit.unitId && state.textRevision === unit.textRevision && state.sourceHash === unit.sourceHash)} onRetry={() => void action.run(() => store.port.retryLexical(unit.unitId, unit.textRevision))} highlight={highlight} onToken={tokenIndex => void action.run(() => store.port.freezeLearning(unit.unitId, unit.textRevision, tokenIndex))}/>
      <div className={styles.translation}><TranslationStack unit={unit} fontPt={snapshot.preferences.translationFontPt * 2 / 3} compact/></div>
      {currentDiffers ? <div className={styles.latest}><span>新台词已到达。此处仍保留原句。</span><Button intent="quiet" disabled={action.pending} onPress={() => { if (snapshot.current) void action.run(() => store.port.freezeLearning(snapshot.current!.unitId, snapshot.current!.textRevision, null)); }}>分析最新一句</Button></div> : null}
      {action.error ? <Notice error>{action.error} 固定原句已保留。</Notice> : null}
      <div role="tabpanel" id={`${tabId}-panel`} aria-labelledby={`${tabId}-${tab}`} className={styles.panel}>
        {tab === 'dictionary' ? <><div className={styles.dictionary}>{lookupError ? <Notice error>{lookupError}</Notice> : lookup ? <DictionaryEntries lookup={lookup}/> : index !== null && index !== undefined ? <p role="status">正在查询选词…</p> : <EmptyState title="选择原文中的一个词语" description="内置词典在本机查询，不需要模型连接。" icon="book"/>}</div>{index !== null && index !== undefined ? <div className={styles.dictionaryActions}><Button disabled={pending.length > 0} pending={action.pending} onPress={() => void request('context_word')}>解释选词</Button><Button intent="quiet" onPress={() => setTab('grammar')}>查看本句精读</Button><small>基础释义来自内置词典；结合本句解释由你主动请求模型。</small></div> : null}</> : <>
          {pending.map(task => <div className={styles.task} key={task.taskId} role="status"><h2>{task.purpose === 'grammar' ? '正在理解这句原文' : '正在解释所选词语'}</h2><p>{task.status === 'queued' ? '请求正在排队。' : '分析对象已固定，你可以继续阅读游戏。'}</p>{task.partialText ? <p className={styles.partial}>{task.partialText}</p> : null}<Button onPress={() => void action.run(() => store.command({ kind: 'cancel_task', payload: { taskId: task.taskId } }))}>取消请求</Button></div>)}
          {tasks.filter(task => task.status === 'failed' || task.status === 'cancelled' || task.status === 'invalidated').map(task => <Notice key={task.taskId} error={task.status === 'failed'}>{task.status === 'failed' ? `解释未完成：${task.errorCode ?? '服务请求失败'}` : task.status === 'cancelled' ? '请求已取消，固定原句保留。' : '配置或授权已变更，旧请求不再生效。'}</Notice>)}
          {results.length ? results.map(result => <StudyExplanation key={result.taskId} result={result} onReference={setHighlight}/>) : !pending.length ? <div className={styles.setup}><span className={styles.setupIcon}><Icon name="book" size={24}/></span><h2>{configured ? '看懂句子结构，而不打断阅读' : '为这类理解选择一个模型'}</h2><p>已保留当前原句。句子精读与实时翻译分别配置，不会自动借用翻译模型。</p>{!configured ? <Button intent="primary" onPress={openSettings}>配置句子精读…</Button> : null}<small>原文与内置词典继续可用</small></div> : null}
          {!pending.length ? <><div className={styles.requestActions}><Button intent={configured ? 'primary' : 'default'} icon="book" pending={action.pending} onPress={() => void request('grammar')}>分析本句文法</Button>{index !== null && index !== undefined ? <Button pending={action.pending} onPress={() => void request('context_word')}>解释选词</Button> : null}</div><p className={styles.requestNote}>解释范围由已应用的用途配置决定；当前固定原句与允许的同会话上下文分别保留。</p></> : null}
        </>}
      </div>
    </div>
    <footer className={styles.footer}><div><strong>{resultProvider ?? (configured ? '句子精读用途已配置' : '尚未选择可用模型')}</strong><small>{pending.length ? '请求进行中，原句保持固定' : '用途独立配置 · 原句与解释分别保留'}</small></div><Button onPress={() => void action.run(() => store.port.closeWindow())}>继续阅读</Button></footer>
  </section>;
}
