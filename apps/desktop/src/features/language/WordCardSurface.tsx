import { useEffect, useState } from 'react';
import type { FrozenWordCard, WordLookup, WordCardIntent } from '../../bridge/contracts.generated';
import type { ApplicationPort } from '../../bridge/application-port';
import { errorMessage } from '../../bridge/application-port';
import { useAction } from '../../bridge/projections';
import { Button } from '../../ui/primitives/Controls';
import { Notice } from '../../ui/patterns/Patterns';
import { DictionaryEntries } from './WordCard';
import styles from './styles.module.css';

/** Shared dictionary content for the native temporary surface and its controlled transport host. */
export function WordCardSurface({ port }: { port: ApplicationPort }) {
  const [card, setCard] = useState<FrozenWordCard | null>(null);
  const [lookup, setLookup] = useState<WordLookup | null>(null);
  const [error, setError] = useState<string | null>(null);
  const action = useAction();
  useEffect(() => {
    let active = true;
    void port.wordCardSnapshot().then(value => {
      if (!active) return; setCard(value);
      document.documentElement.dataset.theme = value.preferences.theme === 'light' || (value.preferences.theme === 'system' && matchMedia('(prefers-color-scheme: light)').matches) ? 'light' : 'dark';
      document.documentElement.dataset.transparency = value.preferences.transparencyMode;
      void port.wordCardLookup(value.cardId).then(result => { if (active) setLookup(result); }).catch(value => { if (active) setError(errorMessage(value)); });
    }).catch(value => { if (active) setError(errorMessage(value)); });
    return () => { active = false; };
  }, [port]);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape' && card) { event.preventDefault(); void port.closeWordCard(card.cardId); } };
    document.addEventListener('keydown', escape); return () => document.removeEventListener('keydown', escape);
  }, [port, card]);
  const perform = (intent: WordCardIntent) => { if (card) void action.run(() => port.wordCardAction(card.cardId, intent)); };
  return <section className={styles.nativeCard} role="dialog" aria-label="本地词典" data-word-card-surface>
    <header><span>固定当前选词</span><Button intent="quiet" icon="x" label="关闭词卡" disabled={!card} onPress={() => { if (card) void port.closeWordCard(card.cardId); }}/></header>
    <div className={styles.nativeCardBody} data-ui-scroll="dictionary">{error ? <Notice error>{error}</Notice> : lookup ? <DictionaryEntries lookup={lookup}/> : <p role="status">正在查询内置词典…</p>}{action.error ? <Notice error>{action.error}</Notice> : null}</div>
    <footer><Button icon="book" disabled={!card} pending={action.pending} onPress={() => perform('learn')}>打开学习窗口</Button><Button icon="sparkles" disabled={!card} pending={action.pending} onPress={() => perform('context_word')}>AI 语境解释</Button></footer>
  </section>;
}
