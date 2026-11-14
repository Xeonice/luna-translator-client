/** Controlled query responses for production HistoryScreen journeys.
 * Not a Repository implementation, native adapter, or evidence of real SQLite. */
import type { CommandEnvelope, Game, HistoryPage, HistoryQuery, HistorySessionSummary, HistorySessionsPage, HistorySessionsQuery, ReadingUnit } from '../../src/bridge/contracts.generated';
import { DesignPort, fixtureUnit } from './design-port';

type Kind = 'records' | 'sessions' | 'detail';
type Fault = { kind: Kind; gameId?: string | null; cursor?: string | null; error?: string; hold?: string };
type WindowPage<T> = { scope: string; owner: string; items: T[]; offset: number };
export function installHistoryFixture(port: DesignPort) {
  const records = new Map<string, WindowPage<ReadingUnit>>(), sessions = new Map<string, WindowPage<HistorySessionSummary>>();
  let cursorSequence = 0;
  const controls = {
    faults: [] as Fault[], pending: new Map<string, () => void>(),
    correctionResponses: [] as { translationId: string; baseRevision: number; correction: { revision: number; text: string } }[],
    recordQueries: [] as HistoryQuery[], sessionQueries: [] as HistorySessionsQuery[], detailQueries: [] as { unitId: string; textRevision: number }[],
    release(key: string) { const resolve = controls.pending.get(key); if (!resolve) throw Error('History hold does not exist'); controls.pending.delete(key); resolve(); },
    async wait(kind: Kind, gameId: string | null, cursor: string | null) {
      const index = controls.faults.findIndex(fault => fault.kind === kind && (fault.gameId === undefined || fault.gameId === gameId) && (fault.cursor === undefined || fault.cursor === cursor));
      if (index < 0) return;
      const fault = controls.faults.splice(index, 1)[0];
      if (fault.hold) await new Promise<void>(resolve => controls.pending.set(fault.hold!, resolve));
      if (fault.error) throw Error(fault.error);
    },
  };
  function page<T>(windows: Map<string, WindowPage<T>>, cursor: string | null, scope: string, limit: number, first: () => T[]): { items: T[]; nextCursor: string | null } {
    const owner = JSON.stringify([port.state.profileId, port.state.serverInstanceId, port.state.profileGeneration]);
    const window = cursor ? windows.get(cursor) : { scope, owner, offset: 0, items: structuredClone(first()) };
    if (!window || window.scope !== scope || window.owner !== owner) throw Error('Controlled history cursor belongs to another owner or scope');
    if (!Number.isInteger(limit) || limit < 1 || limit > 200) throw Error('Controlled history limit invalid');
    const items = window.items.slice(window.offset, window.offset + limit), offset = window.offset + items.length;
    const nextCursor = offset < window.items.length ? `controlled-history-cursor-${++cursorSequence}` : null;
    if (nextCursor) windows.set(nextCursor, { ...window, offset });
    return structuredClone({ items, nextCursor });
  }
  const kindOf = (unit: ReadingUnit) => unit.sourceSummary?.sourceKind ?? 'hook';
  port.historyPage = async query => {
    controls.recordQueries.push(structuredClone(query));
    const filter = query.filter;
    const result = page(records, query.cursor, JSON.stringify(filter), query.limit, () => port.state.history.units.filter(unit => (!filter.gameId || unit.gameId === filter.gameId) && (!filter.sourceKind || kindOf(unit) === filter.sourceKind) && (!filter.sessionId || unit.sessionId === filter.sessionId)));
    await controls.wait('records', filter.gameId, query.cursor);
    return { units: result.items, nextCursor: result.nextCursor } satisfies HistoryPage;
  };
  port.historySessions = async query => {
    controls.sessionQueries.push(structuredClone(query));
    const filter = query.filter;
    const result = page(sessions, query.cursor, JSON.stringify(filter), query.limit, () => {
      const directory = new Map<string, HistorySessionSummary>();
      for (const unit of port.state.history.units) if (unit.sourceSummary && (!filter.gameId || unit.gameId === filter.gameId) && (!filter.sourceKind || kindOf(unit) === filter.sourceKind)) directory.set(unit.sessionId, structuredClone(unit.sourceSummary));
      return [...directory.values()];
    });
    await controls.wait('sessions', filter.gameId, query.cursor);
    return { sessions: result.items, nextCursor: result.nextCursor } satisfies HistorySessionsPage;
  };
  const readUnit = port.readingUnit.bind(port);
  port.readingUnit = async (unitId, textRevision) => {
    controls.detailQueries.push({ unitId, textRevision });
    await controls.wait('detail', null, null);
    return readUnit(unitId, textRevision);
  };
  const dispatch = port.dispatch.bind(port);
  port.dispatch = async (envelope: CommandEnvelope) => {
    if (envelope.command.kind !== 'save_correction' || port.failNextCommand) return dispatch(envelope);
    const response = controls.correctionResponses.shift();
    if (!response) throw Error('No controlled correction response installed');
    const { translationId, baseRevision, text } = envelope.command.payload;
    if (response.translationId !== translationId || response.baseRevision !== baseRevision || response.correction.text !== text) throw Error('Correction command does not match the prepared response');
    port.commands.push(structuredClone(envelope));
    const units = structuredClone(port.state.history.units);
    const translation = units.flatMap(unit => unit.translations).find(item => item.translationId === translationId);
    if (!translation) throw Error('Prepared correction target missing');
    translation.correction = structuredClone(response.correction);
    port.publish({ history: { ...port.state.history, units }, commitRevision: port.state.commitRevision + 1 });
    return { requestId: envelope.operationId, revision: port.state.commitRevision, replayed: false, resultId: translationId };
  };
  const game = (id: string, title: string): Game => ({ gameId: id, displayName: title, originalTitle: null, sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1, target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } });
  const base = fixtureUnit();
  const units = Array.from({ length: 139 }, (_, index): ReadingUnit => {
    const gameId = index < 65 ? 'history-game-a' : index < 135 ? 'history-game-b' : null;
    const sessionId = gameId === 'history-game-a' ? `history-session-a-${index}` : gameId === 'history-game-b' ? `history-session-b-${index % 3}` : 'history-manual-session';
    const createdAtMs = base.createdAtMs - index * 1000;
    return { ...structuredClone(base), unitId: `history-unit-${index}`, gameId, sessionId, sourceText: `${gameId === 'history-game-a' ? '甲游戏记录' : gameId === 'history-game-b' ? '乙游戏记录' : '手动输入记录'} ${index}`, tokens: [], createdAtMs, sourceSummary: { sessionId, gameId, sourceKind: gameId ? 'hook' : 'manual', state: 'stopped', startedAtMs: createdAtMs, endedAtMs: createdAtMs }, translations: [{ translationId: `history-translation-${index}`, taskId: `history-task-${index}`, text: `对应译文 ${index}`, providerLabel: '历史测试引擎', targetLanguage: 'zh-CN', correction: null }] };
  });
  port.publish({ games: [game('history-game-a', '甲游戏'), game('history-game-b', '乙游戏')], history: { units, nextCursor: null }, current: units[120] });
  return controls;
}
