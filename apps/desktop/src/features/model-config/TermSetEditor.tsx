import { ConfigurationRow as FormRow, ConfigurationSwitch as Switch } from './ConfigurationFields';
import type { Game, Term, TermSet } from '../../bridge/contracts.generated';
import { Button, Select, TextField } from '../../ui/primitives/Controls';
import { Group, Notice } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';

export function TermSetEditor({ termSet, games, onChange }: { termSet: TermSet; games: Game[]; onChange: (value: TermSet) => void }) {
  const patch = (value: Partial<TermSet>) => onChange({ ...termSet, ...value });
  const editTerm = (termId: string, value: Partial<Term>) => patch({ terms: termSet.terms.map(term => term.termId === termId ? { ...term, ...value } : term) });
  return <>
    <Group title="术语与作用范围" note="术语作为参考资料，不作为系统命令执行。保存草稿、检查与应用分别操作。">
      <FormRow label="术语表名称"><TextField label="术语表名称" value={termSet.displayName} onChange={displayName => patch({ displayName })}/></FormRow>
      <FormRow label="作用范围" note="作用范围独立保存；已应用用途在检查并确认后才更新。"><Select label="术语作用范围" value={termSet.scopeKey} onChange={scopeKey => patch({ scopeKey, gameId: scopeKey === 'global' ? null : scopeKey.slice(5) })}><option value="global">全局默认</option>{games.map(game => <option key={game.gameId} value={`game:${game.gameId}`}>{game.displayName} · 仅此游戏</option>)}</Select></FormRow>
      <FormRow label="使用状态"><Switch label="启用此术语表" selected={termSet.enabled} onChange={enabled => patch({ enabled })} description="只将已启用且匹配当前原句的完整词项作为参考材料。"/></FormRow>
    </Group>
    <section className={styles.termsSection}><header className={styles.termsHeading}><h2>词项</h2><Button onPress={() => patch({ terms: [...termSet.terms, { termId: crypto.randomUUID(), sourceText: null, targetText: null, sourceLanguage: null, targetLanguage: null, enabled: true }] })}>添加词项</Button></header><div className={styles.termsBody}>
      {!termSet.terms.length ? <Notice>还没有词项。可添加人物称呼、专有名词及约定译法。</Notice> : null}
      {termSet.terms.map((term, index) => <section key={term.termId} className={styles.termCard} aria-label={`术语项 ${index + 1}`}>
        <div className={styles.termFields}><TextField label={`原词 ${index + 1}`} value={term.sourceText ?? ''} onChange={sourceText => editTerm(term.termId, { sourceText: sourceText || null })}/><TextField label={`译法 ${index + 1}`} value={term.targetText ?? ''} onChange={targetText => editTerm(term.termId, { targetText: targetText || null })}/></div>
        <div className={styles.termActions}><Switch label={`启用词项 ${index + 1}`} selected={term.enabled} onChange={enabled => editTerm(term.termId, { enabled })}/><Button label={`从草稿移除词项 ${index + 1}`} onPress={() => patch({ terms: termSet.terms.filter(item => item.termId !== term.termId) })}>从草稿移除</Button></div>
      </section>)}
    </div><p className={styles.termsDescription}>不完整词项可保留在草稿中；不会进入实际请求。移除词项保留其他词项的身份与顺序。已有语言元数据原样保留。</p></section>
  </>;
}
