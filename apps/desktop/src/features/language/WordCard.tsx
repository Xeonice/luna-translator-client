import { Fragment, useEffect, useRef, useState } from 'react';
import type { ReadingUnit, WordLookup } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { Button } from '../../ui/primitives/Controls';
import { Notice, StatusLabel } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';

function humanUsageNotes(notes: string | null): string | null {
  const text = notes?.trim();
  if (!text) return null;
  // Imported metadata is not prose. Do not expose opaque tags or source identifiers.
  if (text.startsWith('{') || text.startsWith('[')) return null;
  try { JSON.parse(text); return null; } catch { return text; }
}

export function DictionaryEntries({ lookup }: { lookup: WordLookup }) {
  return <>
    <h3 lang="ja">{lookup.token.lemma || lookup.token.surface}</h3>
    <p>{lookup.token.reading} · {lookup.token.partOfSpeech}</p><dl className={styles.wordFacts}><div><dt>词面</dt><dd>{lookup.token.surface}</dd></div><div><dt>词元</dt><dd>{lookup.token.lemma || lookup.token.surface}</dd></div></dl>
    {lookup.entries.length ? <ul className={styles.entries}>{lookup.entries.map(entry => <li key={entry.entryId}>
      <article aria-label={`词典词条 ${entry.lemma}`}>
        <header><h4 lang="ja">{entry.lemma}</h4>{entry.reading ? <p lang="ja">{entry.reading}</p> : null}</header>
        {entry.senses.map((sense, index) => {
          const usage = humanUsageNotes(sense.usageNotes);
          return <Fragment key={index}>
            <StatusLabel>{sense.language === 'zh' || sense.language.startsWith('zh-') ? '中文释义' : `${sense.language} 补充释义`}</StatusLabel>
            <p>{sense.text}</p>{usage ? <small className={styles.usage}>{usage}</small> : null}
          </Fragment>;
        })}
        <small>来源：{entry.sourceLabel} · {entry.sourceVersion}</small>
        <details><summary>许可与署名</summary><p>{entry.licenseRef}<br/>{entry.attribution}</p></details>
      </article>
    </li>)}</ul> : <p className={styles.emptyDefinition}>内置词典暂未收录这个词。原文与分词仍然保留，可主动请求语境解释。</p>}
    <small>{`本地词典 · ${lookup.resourceVersion}`}</small>
  </>;
}

/** Opens a platform-owned surface; no dictionary content is clipped by the HUD webview. */
export function WordCard({ unit, tokenIndex, anchor, store, onClose }: { unit: ReadingUnit; tokenIndex: number; anchor: HTMLButtonElement; store: ApplicationStore; onClose: () => void }) {
  const closeRef = useRef(onClose); closeRef.current = onClose;
  useEffect(() => {
    let active = true; let cardId: string | null = null; let stop: (() => void) | undefined;
    const close = () => { if (cardId) void store.port.closeWordCard(cardId).catch(() => {}); closeRef.current(); };
    const rect = anchor.getBoundingClientRect();
    const closedBeforeReply = new Set<string>();
    void (async () => {
      const dispose = await store.port.onWordCardClosed(closed => { if (!active) return; if (closed === cardId) closeRef.current(); else closedBeforeReply.add(closed); });
      if (!active) { dispose(); return; } stop = dispose;
      const id = await store.port.openWordCard({ unitId: unit.unitId, textRevision: unit.textRevision, tokenIndex, anchorRect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height } });
      cardId = id;
      if (!active) { await store.port.closeWordCard(id); return; }
      if (closedBeforeReply.has(id)) closeRef.current();
    })().catch(value => { if (active) setError(errorMessage(value)); });
    // The selection and rect are frozen. A new source sentence may replace this DOM node.
    const scrollOwners = new Set<Element>();
    for (let element = anchor.parentElement; element; element = element.parentElement) scrollOwners.add(element);
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') close(); };
    const outside = (event: PointerEvent) => { if (event.target instanceof Element && !event.target.closest('[data-word-card-surface]') && !anchor.contains(event.target)) close(); };
    document.addEventListener('keydown', escape); document.addEventListener('pointerdown', outside);
    const scrolled = (event: Event) => { if (event.target === document || (event.target instanceof Element && scrollOwners.has(event.target))) close(); };
    // Native owner geometry is classified by Rust. Browser resize also fires
    // for content-fit, so it cannot independently invalidate a frozen word.
    document.addEventListener('scroll', scrolled, true);
    return () => { active = false; stop?.(); if (cardId) void store.port.closeWordCard(cardId).catch(() => {}); document.removeEventListener('keydown', escape); document.removeEventListener('pointerdown', outside); document.removeEventListener('scroll', scrolled, true); };
  }, [store, unit.unitId, unit.textRevision, tokenIndex, anchor]);
  const [error, setError] = useState<string | null>(null);
  return error ? <Notice error>{error}<Button intent="quiet" onPress={() => closeRef.current()}>关闭提示</Button></Notice> : null;
}
