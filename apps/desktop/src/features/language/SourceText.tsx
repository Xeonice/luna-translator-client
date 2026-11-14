import { Fragment } from 'react';
import type { ReactNode } from 'react';
import type { AnalysisReference, LexicalState, Preferences, ReadingUnit } from '../../bridge/contracts.generated';
import styles from './styles.module.css';

function tokenLayout(unit: ReadingUnit): { gaps: string[]; tail: string } | null {
  if (!unit.tokens.length) return null;
  const encoder = new TextEncoder();
  const gaps: string[] = [];
  let utf16Cursor = 0; let byteCursor = 0;
  for (const token of unit.tokens) {
    if (![token.utf16Start, token.utf16End, token.byteStart, token.byteEnd].every(Number.isSafeInteger)
      || token.utf16Start < utf16Cursor || token.utf16End <= token.utf16Start || token.utf16End > unit.sourceText.length) return null;
    const gap = unit.sourceText.slice(utf16Cursor, token.utf16Start);
    if (/\S/u.test(gap) || unit.sourceText.slice(token.utf16Start, token.utf16End) !== token.surface) return null;
    const start = byteCursor + encoder.encode(gap).length;
    const end = start + encoder.encode(token.surface).length;
    if (token.byteStart !== start || token.byteEnd !== end) return null;
    gaps.push(gap); utf16Cursor = token.utf16End; byteCursor = end;
  }
  const tail = unit.sourceText.slice(utf16Cursor);
  return /\S/u.test(tail) || byteCursor + encoder.encode(tail).length !== encoder.encode(unit.sourceText).length ? null : { gaps, tail };
}

function typographicGroups(unit: ReadingUnit, gaps: string[]): number[][] {
  const groups: number[][] = [];
  for (let index = 0; index < unit.tokens.length; index += 1) {
    const previous = unit.tokens[index - 1];
    // Buttons are atomic inline controls, so browser kinsoku alone cannot keep
    // a following closing mark with its word. Join typography without merging
    // lexical tokens, crossing original whitespace or changing lookup indices.
    const closing = /^[\p{Pe}\p{Pf}、。，．？！!?：:；;…‥]+$/u.test(unit.tokens[index].surface);
    const afterOpening = previous && /^[\p{Ps}\p{Pi}]+$/u.test(previous.surface);
    if (previous && !/\s/u.test(previous.surface) && !gaps[index] && (closing || afterOpening)) groups[groups.length - 1].push(index);
    else groups.push([index]);
  }
  return groups;
}

const lexicalMessages = { pending: '正在准备本机词法标注，原文已保留。', resource_unavailable: '本机词法资源暂不可用，原文已保留。', worker_failed: '词法暂不可用，原文已保留。注音与词性待恢复后显示。', alignment_invalid: '词法结果与原文未对齐，原文已保留。', reading_unavailable: '部分词语读音未知，保留原字，不推测注音。', ready: '' };
type SourceStatusProps = { unit: ReadingUnit; preferences: Preferences; lexicalState?: LexicalState; onRetry?: () => void };
/** Shared state notice can follow translations in the HUD while other reading
 * surfaces keep it beside their source. Identity rules are identical. */
export function SourceTextStatus({ unit, preferences, lexicalState, onRetry }: SourceStatusProps) {
  const lexical = lexicalState?.unitId === unit.unitId && lexicalState.textRevision === unit.textRevision && lexicalState.sourceHash === unit.sourceHash ? lexicalState : undefined;
  if (!(preferences.segmentationEnabled || preferences.showRuby) || unit.sourceLanguage !== 'ja') return null;
  const message = lexical ? lexicalMessages[lexical.state] : unit.tokens.length && !tokenLayout(unit) ? lexicalMessages.alignment_invalid : '';
  return message ? <p className={styles.lexicalNotice} role="status" data-lexical-state={lexical?.state ?? 'alignment_invalid'}>{message}{onRetry && lexical?.state !== 'pending' && lexical?.state !== 'reading_unavailable' ? <button type="button" onClick={onRetry}>重试分词</button> : null}</p> : null;
}

export function SourceText({ unit, preferences, onToken, highlight, lexicalState, onRetry, showStatus = true }: { unit: ReadingUnit; preferences: Preferences; onToken?: (index: number, anchor: HTMLButtonElement) => void; highlight?: Pick<AnalysisReference, 'byteStart' | 'byteEnd'> | null; lexicalState?: LexicalState; onRetry?: () => void; showStatus?: boolean }) {
  // Old immutable snapshots may omit whitespace nodes. Preserve those gaps as
  // plain text without manufacturing token identities or changing lookup indices.
  const segmentation = preferences.segmentationEnabled;
  // Analysis identity is independent of presentation. Ruby still needs the
  // validated spans when clickable segmentation is switched off.
  const lexical = lexicalState?.unitId === unit.unitId && lexicalState.textRevision === unit.textRevision && lexicalState.sourceHash === unit.sourceHash ? lexicalState : undefined;
  const lexicalNeeded = segmentation || preferences.showRuby;
  const lexicalReady = !lexical || lexical.state === 'ready' || lexical.state === 'reading_unavailable';
  const layout = lexicalNeeded && lexicalReady ? tokenLayout(unit) : null;
  let plain = <>{unit.sourceText}</>;
  if (!layout && highlight) {
    try { const bytes = new TextEncoder().encode(unit.sourceText); const decoder = new TextDecoder('utf-8', { fatal: true }); if (highlight.byteStart >= 0 && highlight.byteEnd <= bytes.length && highlight.byteStart < highlight.byteEnd) plain = <>{decoder.decode(bytes.slice(0, highlight.byteStart))}<mark className={styles.highlight}>{decoder.decode(bytes.slice(highlight.byteStart, highlight.byteEnd))}</mark>{decoder.decode(bytes.slice(highlight.byteEnd))}</>; } catch { /* Malformed view range has no highlight authority. */ }
  }
  const renderToken = (index: number) => {
    const token = unit.tokens[index];
    const selected = highlight && token.byteStart < highlight.byteEnd && token.byteEnd > highlight.byteStart;
    const posClass = /^(名词|名詞|noun|n)$/i.test(token.partOfSpeech) ? styles.posNoun : /^(动词|動詞|verb|v)$/i.test(token.partOfSpeech) ? styles.posVerb : /^(助词|助詞|particle)$/i.test(token.partOfSpeech) ? styles.posParticle : ''; 
    // Unicode letters, marks and numbers can form a lookup word; punctuation and whitespace stay in the text flow.
    // Keeping punctuation as inline text lets the browser apply Japanese line-breaking rules across token boundaries.
    const content = !/[\p{L}\p{M}\p{N}]/u.test(token.surface)
      ? <span key={index} className={selected ? styles.highlight : undefined} data-token-index={index}>{token.surface}</span>
      : !segmentation ? preferences.showRuby && token.reading && token.reading !== token.surface ? <ruby key={index} className={`${styles.rubyToken} ${selected ? styles.highlight : ''}`} data-token-index={index}>{token.surface}<rt>{token.reading}</rt></ruby> : <Fragment key={index}>{selected ? <mark className={styles.highlight}>{token.surface}</mark> : token.surface}</Fragment>
        : <button key={index} type="button" className={`${styles.token} ${preferences.showPos && posClass ? `${styles.pos} ${posClass}` : ''}`} data-token-index={index} data-pos={token.partOfSpeech} data-highlighted={selected ? true : undefined} onClick={event => onToken?.(index, event.currentTarget)} disabled={!onToken} aria-label={`${token.surface}${token.reading ? `，读音 ${token.reading}` : ''}`}><ruby>{token.surface}{preferences.showRuby && token.reading && token.reading !== token.surface ? <rt>{token.reading}</rt> : null}</ruby></button>;
    return content;
  };
  const unsegmentedContent = (): ReactNode[] => {
    const content: ReactNode[] = []; let text = '';
    const flush = () => { if (text) { content.push(text); text = ''; } };
    unit.tokens.forEach((token, index) => {
      text += layout?.gaps[index] ?? '';
      const selected = highlight && token.byteStart < highlight.byteEnd && token.byteEnd > highlight.byteStart;
      if (selected || preferences.showRuby && token.reading && token.reading !== token.surface) { flush(); content.push(renderToken(index)); }
      else text += token.surface;
    });
    text += layout?.tail ?? ''; flush(); return content;
  };
  return <><div className={styles.source} lang={unit.sourceLanguage} style={{ fontSize: `${preferences.sourceFontPt}px` }} data-testid="source-text" data-segmentation={segmentation} data-ruby={preferences.showRuby} data-pos={segmentation && preferences.showPos}>{layout ? <span className={preferences.showRuby ? styles.rubySource : undefined}>{segmentation ? typographicGroups(unit, layout.gaps).map(group => <Fragment key={unit.tokens[group[0]].utf16Start}>{layout.gaps[group[0]]}{group.length > 1 ? <span className={styles.typographicGroup}>{group.map(renderToken)}</span> : renderToken(group[0])}</Fragment>) : unsegmentedContent()}{segmentation ? layout.tail : null}</span> : plain}</div>{showStatus ? <SourceTextStatus unit={unit} preferences={preferences} lexicalState={lexicalState} onRetry={onRetry}/> : null}</>;
}
