/** Typed, development-only authored reading data. No business I/O or Native proof. */
import type { Game, LexicalToken, ReadingUnit, SavedServiceRef, TaskView } from '../../src/bridge/contracts.generated';
import type { DesignPort } from '../support/design-port';

export const DESIGN_READING_SAMPLE = {
  id: 'design-reference-reading-sample-v1',
  provenance: {
    source: 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html',
    sourceSha256: '1a8269771e23e2ecfb577e7666409947dd32ce50e135940d90b7d6e11d23ecaa',
    dataSource: 'docs/design/first-product-v1/data.js',
    dataSourceSha256: '5ebce1b36e9f6b17a14b6f232f8bd1760ce0a8765a3138592bb8f194b62b26f9',
    comparisonRecipe: '.build/current-client-execution-20261105/live-ui-preview/visual-comparison/source-recipe.json',
    scope: 'Design comparison memory sample; no service qualification or captured frame.',
  },
  game: { id: 'sakura', title: 'サクラノ詩 10th Anniversary edition', bottle: 'Sakura-no-Uta-10th', executable: 'sakuranouta10th.exe', architecture: 'x64', targetPath: 'C:\\\\Games\\\\SakuraNoUta10th\\\\sakuranouta10th.exe', hook: 'Artemis' },
  source: 'その風景には、悲しさも、痛みも、なかった。',
  sourceSha256: 'f7d2690f72c8a38a5aaa370a1f43deac68f4c52de359ec8fd431da41dc9895e5',
  translation: '那片景色中，没有悲伤，也没有痛楚。',
  provider: 'Microsoft · 日中翻译',
  time: '00:31',
  tokens: [
    ['その', '', 'その', '连体词'], ['風景', 'ふうけい', '風景', '名词'], ['には', '', 'には', '助词'], ['、', '', '、', '原文'],
    ['悲しさ', 'かなしさ', '悲しい', '名词'], ['も', '', 'も', '助词'], ['、', '', '、', '原文'], ['痛み', 'いたみ', '痛み', '名词'],
    ['も', '', 'も', '助词'], ['、', '', '、', '原文'], ['なかった', '', 'ない', '助动词'], ['。', '', '。', '原文'],
  ],
} as const;

export async function installDesignReadingSample(port: DesignPort): Promise<void> {
  const previous = port.state.current, session = port.state.session;
  if (!previous || !session) throw new Error('设计对照样例需要受控阅读上下文。');
  if (DESIGN_READING_SAMPLE.tokens.map(([surface]) => surface).join('') !== DESIGN_READING_SAMPLE.source) throw new Error('设计对照样例的词语跨度与原文不一致。');
  const sourceHash = [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(DESIGN_READING_SAMPLE.source)))].map(byte => byte.toString(16).padStart(2, '0')).join('');
  if (sourceHash !== DESIGN_READING_SAMPLE.sourceSha256) throw new Error('设计对照样例原文身份发生变化。');
  let byte = 0, utf16 = 0;
  const tokens: LexicalToken[] = DESIGN_READING_SAMPLE.tokens.map(([surface, reading, lemma, partOfSpeech]) => {
    const byteStart = byte, utf16Start = utf16; byte += new TextEncoder().encode(surface).length; utf16 += surface.length;
    return { surface, reading, lemma, partOfSpeech, byteStart, byteEnd: byte, utf16Start, utf16End: utf16, known: true };
  });
  const createdAtMs = new Date(2026, 9, 19, 0, 31).getTime();
  const taskId = 'design-sample-completed-translation';
  const unit: ReadingUnit = {
    ...structuredClone(previous), unitId: 'design-sample-reading-unit', gameId: DESIGN_READING_SAMPLE.game.id, sourceText: DESIGN_READING_SAMPLE.source,
    sourceHash, tokens, sourceLanguage: 'ja', createdAtMs,
    sourceSummary: { sessionId: session.sessionId, gameId: DESIGN_READING_SAMPLE.game.id, sourceKind: 'hook', state: 'active', startedAtMs: createdAtMs, endedAtMs: null },
    translations: [{ translationId: 'design-sample-translation', taskId, text: DESIGN_READING_SAMPLE.translation, providerLabel: DESIGN_READING_SAMPLE.provider, targetLanguage: 'zh-CN', correction: null }],
  };
  const task: TaskView = { taskId, unitId: unit.unitId, textRevision: unit.textRevision, purpose: 'translation', status: 'completed', errorCode: null, partialText: null, providerLabel: DESIGN_READING_SAMPLE.provider, attemptId: 'design-sample-completed-attempt' };
  const commitRevision = port.state.commitRevision + 1;
  port.readingProjectionFixtures[JSON.stringify([unit.unitId, unit.textRevision])] = {
    profileId: port.state.profileId, serverInstanceId: port.state.serverInstanceId, profileGeneration: port.state.profileGeneration, commitRevision, unit: structuredClone(unit),
    translationState: { unitId: unit.unitId, textRevision: unit.textRevision, sourceHash, engines: [{ scopeKey: 'global', bindingId: 'design-sample-binding', engineId: 'design-sample-engine', providerLabel: DESIGN_READING_SAMPLE.provider, status: { kind: 'current', task } }] },
  };
  const source: SavedServiceRef = { kind: 'http', source: { revisionId: 'design-sample-configuration', modelId: 'design-sample-microsoft' } };
  port.serviceFixtures.saved = async query => ({
    profileGeneration: port.state.profileGeneration, appliedHeadRevision: 1,
    selection: { requestedScope: structuredClone(query.scope), effectiveScope: { kind: 'global' }, selectionRevision: 1, effectiveSelectionRevision: 1, main: query.purpose === 'translation' ? source : null, comparisons: [], purposeEnabled: false },
    rows: query.purpose === 'translation' ? [{ source, logicalResourceId: 'design-sample-microsoft', displayName: DESIGN_READING_SAMPLE.provider, backend: 'http_translation', purpose: 'translation',
      eligibility: { kind: 'blocked', reasons: [{ code: 'design_sample_no_service_authority', message: '设计对照内存样例，不提供真实服务调用资格。', fieldId: null }] }, verification: { kind: 'untested' } }] : [],
  });
  const game: Game = { gameId: DESIGN_READING_SAMPLE.game.id, displayName: DESIGN_READING_SAMPLE.game.title, originalTitle: null, sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1,
    target: { environmentKind: 'crossover', environmentKey: 'design-sample-environment', bottleLocator: DESIGN_READING_SAMPLE.game.bottle, launchPath: DESIGN_READING_SAMPLE.game.targetPath, canonicalExePath: DESIGN_READING_SAMPLE.game.targetPath, targetPathKey: null, workingDirectory: null, arguments: [], architecture: DESIGN_READING_SAMPLE.game.architecture, verificationState: 'unresolved' } };
  // No made-up screenshot, permission, selected window or collector success.
  port.bindingFixture = { state: 'unbound', sessionId: session.sessionId, targetIdentitySha256: null, windowTitle: null, verifiedAtMs: null, errorCode: null, detail: '设计对照内存样例不提供实际运行窗口或录屏事实。' };
  port.previewProjections.clear(); port.previewFrameFixtures.clear();
  port.previewCapabilityFixture = { supported: false, permission: 'not_requested', errorCode: 'design_sample_no_native', detail: '设计对照内存样例没有原生采集能力。' };
  port.publish({ games: [game], current: unit, history: { units: [unit], nextCursor: null }, session: { ...session, gameId: game.gameId, state: 'active', paused: false, translationEnabled: true,
    sources: [{ sourceId: 'design-sample-source', hookName: DESIGN_READING_SAMPLE.game.hook, hookCode: 'design-sample', sample: DESIGN_READING_SAMPLE.source, selected: true, encoding: 'UTF-16' }] },
    lexicalStates: [{ unitId: unit.unitId, textRevision: unit.textRevision, sourceHash, state: 'ready', errorCode: null }], tasks: [task], commitRevision,
    ...(port.state.learningObject ? { learningObject: { unit: structuredClone(unit), tokenIndex: null } } : {}),
  });
}
