/** UI-only controlled transport. This module is never imported by the production bootstrap. */
import type { AppSnapshot, AssociateRequest, CommandEnvelope, Game, GameSaveResult, Configuration, ConfigurationTestRequest, CredentialInput, CredentialReference, CredentialRevocationImpact, CredentialRevocationRequest, ConfigurationState, DictionaryEntry, ExecutableSelection, HistoryPage, ProjectionUpdate, ReadingUnit, TargetCandidate, HistoryQuery, HistorySessionsQuery, HistorySessionsPage, HistorySessionSummary, OpenWordCardRequest, FrozenWordCard, WordCardIntent } from '../../src/bridge/contracts.generated';
import type * as Contracts from '../../src/bridge/contracts.generated';
import type { ApplicationPort, WindowRole } from '../../src/bridge/application-port';

const sentence = '放課後、図書館で彼女と待ち合わせをした。';
export function fixtureUnit(): ReadingUnit {
  let byte = 0; let utf16 = 0;
  const tokens = [['放課後', 'ほうかご', '名詞'], ['、', '', '補助記号'], ['図書館', 'としょかん', '名詞'], ['で', '', '助詞'], ['彼女', 'かのじょ', '代名詞'], ['と', '', '助詞'], ['待ち合わせ', 'まちあわせ', '名詞'], ['を', '', '助詞'], ['した', '', '動詞'], ['。', '', '補助記号']].map(([surface, reading, partOfSpeech]) => { const byteStart = byte; const utf16Start = utf16; byte += new TextEncoder().encode(surface).length; utf16 += surface.length; return { surface, reading, lemma: surface, partOfSpeech, byteStart, byteEnd: byte, utf16Start, utf16End: utf16, known: true }; });
  return { sourceSummary: { sessionId: 'ui-fixture-session', gameId: null, sourceKind: 'manual', state: 'stopped', startedAtMs: Date.UTC(2026, 9, 19, 10, 30), endedAtMs: Date.UTC(2026, 9, 19, 10, 30) }, unitId: 'ui-fixture-unit-1', sessionId: 'ui-fixture-session', gameId: null, textRevision: 1, sourceText: sentence, sourceHash: 'a'.repeat(64), sourceLanguage: 'ja', createdAtMs: Date.UTC(2026, 9, 19, 10, 30), translations: [], tokens };
}
export function emptySnapshot(): AppSnapshot { return { lexicalStates: [], profileId: 'ui-test-profile', serverInstanceId: 'ui-test-instance', profileGeneration: 1, revision: 0, commitRevision: 0, games: [], session: null, current: null, history: { units: [], nextCursor: null }, configuration: { draft: { kind: 'legacy', configuration: { revisionId: '', connections: [], models: [], bindings: [], templates: [], termSets: [] } }, applied: null, draftHeadRevision: 0, appliedHeadRevision: 0, runtimeConnections: [], runtimePurposes: [] }, preferences: { segmentationEnabled: true, readingDisplayMode: 'both', hudBackgroundOpacity: .72, scopeKey: 'global', theme: 'dark', transparencyMode: 'material', showRuby: true, showPos: true, sourceFontPt: 18, translationFontPt: 24, hudWidthPt: 560, shortcuts: {}, overrides: {} }, diagnostics: [], tasks: [], studyResults: [], configurationTests: [], credentials: [], credentialPools: [], learningObject: null }; }
function modelConfigFixture(): Configuration {
  return {
    revisionId: 'ui-config-supported',
    connections: [{ connectionId: 'ui-connection-a', displayName: 'UI configured service', protocol: 'openai_compatible', deployment: 'local', authMode: 'none', endpoint: 'http://127.0.0.1:1234/v1', credentialPoolId: null, credentialPoolRevision: null, proxyMode: 'inherit', networkMetadata: { region: 'ui-preserved-region' }, availabilityIntent: true }],
    models: ['A', 'B'].map(label => ({ modelId: `ui-model-${label.toLowerCase()}`, connectionId: 'ui-connection-a', actualModelId: `ui-entered-${label}`, displayName: `UI model ${label}`, selectionSource: 'manual', capabilities: {}, parameters: {}, defaultTemplates: {} })),
    bindings: [{ bindingId: 'ui-binding-translation', scopeKey: 'global', gameId: null, purpose: 'translation', bindingMode: 'override', enabled: false, outputMode: 'text', sourceLanguage: 'ja', targetLanguage: 'zh-CN', templateId: null, context: { enabled: false, maxSentences: 0 }, schedule: { timeoutMs: 60000, retries: 0 }, parameters: {}, engines: [{ engineId: 'ui-engine-a', modelId: 'ui-model-a', enabled: true, templateId: null, parameters: { temperature: { mode: 'set', value: 0 } } }, { engineId: 'ui-engine-b', modelId: 'ui-model-b', enabled: true, templateId: null, parameters: {} }] }],
    templates: [], termSets: [],
  };
}
export class DesignPort implements ApplicationPort {
  /** Explicit transport fixtures only; no target relation or successful commit is inferred here. */
  bindingFixtures: {
    review?: (query: Contracts.AssociationReviewQuery) => Promise<Contracts.AssociationReview>;
    associate?: (request: Contracts.ReviewedAssociateRequest) => Promise<Contracts.AssociationOperationReceipt>;
    progress?: (query: Contracts.AssociationProgressQuery) => Promise<Contracts.AssociationProgress>;
    sources?: (query: Contracts.SourceSelectionQuery) => Promise<Contracts.SourceSelectionProjection>;
    begin?: (request: Contracts.BeginReadingRequest) => Promise<Contracts.BeginReadingResult>;
    effects?: (authority: Contracts.ReadingWindowEffectAuthority) => Promise<Contracts.ReadingWindowEffectsResult>;
  } = {};
  reviewedAssociations: { request: Contracts.ReviewedAssociateRequest; review: Contracts.AssociationReview }[] = [];
  bindingJourneyFixture = { relation: 'none' as Contracts.AssociationRelation, stage: 'waiting_text' as Contracts.AssociationStage, sequence: 1, setRevision: 0, autoMatchGameId: null as string | null, beginRequests: [] as Contracts.BeginReadingRequest[] };
  /** Opt-in scripted DTO boundaries for legacy UI regressions, never native/SQLite evidence. */
  prepareBindingJourneyFixture(relation: Contracts.AssociationRelation = 'none') {
    this.bindingJourneyFixture.relation = relation;
    const reviews = new Map<string, Contracts.AssociationReview>();
    const profile = () => ({ profileId: this.state.profileId, serverInstanceId: this.state.serverInstanceId, profileGeneration: this.state.profileGeneration });
    this.bindingFixtures.review = async query => {
      const candidate = this.targetCandidates.find(candidate => candidate.candidateId === query.candidateId);
      if (!candidate) throw new Error('Controlled review has no prepared candidate');
      const review: Contracts.AssociationReview = { reviewId: crypto.randomUUID(), profile: profile(), relation: this.bindingJourneyFixture.relation, candidate: { candidateId: candidate.candidateId, environmentKey: candidate.environmentKey, executablePath: candidate.executablePath, windowsPid: candidate.windowsPid, processCreatedU64: candidate.processCreatedU64, architecture: candidate.architecture, targetIdentitySha256: 'f'.repeat(64) }, gameId: query.attributionIntent === 'auto_match' ? this.bindingJourneyFixture.autoMatchGameId : query.gameId, activeSessionId: query.expectedActiveSessionId, activeTargetIdentitySha256: null, detail: 'Explicit controlled reviewed DTO; not a runtime identity decision.', error: null };
      reviews.set(review.reviewId, review); return review;
    };
    this.bindingFixtures.associate = async request => {
      const review = reviews.get(request.reviewId); if (!review) throw new Error('Controlled reviewed request has no prepared review');
      this.reviewedAssociations.push({ request: structuredClone(request), review: structuredClone(review) });
      if (this.holdAssociation) await new Promise<void>(resolve => { this.associationResume = resolve; });
      const response = this.associationResponses.shift(); if (response?.error) throw new Error(response.error);
      this.publish({ session: { sessionId: 'ui-fixture-session', gameId: review.gameId, sourceKind: 'hook', state: 'active', paused: false, translationEnabled: true, sources: [] } });
      return { operationId: request.operationId, reviewId: request.reviewId, profile: profile(), relation: review.relation, sessionId: 'ui-fixture-session', replayed: false };
    };
    this.bindingFixtures.progress = async query => ({ operationId: query.operationId, profile: profile(), sessionId: this.state.session?.sessionId ?? null, generation: 'controlled-binding-generation', stage: this.bindingJourneyFixture.stage, facts: [{ eventId: 'controlled-stage-' + this.bindingJourneyFixture.sequence, sequence: String(this.bindingJourneyFixture.sequence), observedAtMs: '1792368000000', stage: this.bindingJourneyFixture.stage, level: 'info', code: 'controlled_stage_input', message: 'Explicit controlled stage input; not a Core event.' }], superseded: false, error: null });
    this.bindingFixtures.sources = async query => ({ profile: profile(), sessionId: query.sessionId, state: 'unselected', current: null, sources: (this.state.session?.sources ?? []).map(source => ({ sourceId: source.sourceId, hookName: source.hookName, hookCode: source.hookCode, address: null, context: null, context2: null, encoding: source.encoding, latestSample: source.sample, streamIdentitySha256: 'f'.repeat(64), identityExtensionsJson: '{"controlled_fixture":true}' })), saved: null, rememberAllowed: this.state.session?.gameId != null, memberLimit: 64 });
    this.bindingFixtures.begin = async request => {
      this.bindingJourneyFixture.beginRequests.push(structuredClone(request));
      const setRevision = String(++this.bindingJourneyFixture.setRevision);
      const selection: Contracts.SourceSelectionSet = { sessionId: request.sessionId, setRevision, operationId: request.operationId, persistence: request.remember ? 'saved_snapshot' : 'temporary', selectedAtMs: '1792368000000', members: request.sourceIds.map((sourceId, ordinal) => ({ sourceId, ordinal, selectionRevision: this.bindingJourneyFixture.setRevision })) };
      const authority = { profile: profile(), sessionId: request.sessionId, setRevision, operationId: request.operationId };
      if (this.state.session) this.publish({ session: { ...this.state.session, sources: this.state.session.sources.map(source => ({ ...source, selected: request.sourceIds.includes(source.sourceId) })) } });
      return { selection: { operationId: request.operationId, profile: profile(), selection, replayed: false, effectAuthority: authority }, effects: { authority, showHud: { state: 'succeeded', error: null }, hideMain: { state: 'succeeded', error: null }, retryable: false, detail: 'Explicit controlled effects receipt; no native window was changed.' } };
    };
  }
  async reviewAssociation(query: Contracts.AssociationReviewQuery) { if (!this.bindingFixtures.review) throw new Error('受控后端尚未安装 Rust 目标核对投影。'); return structuredClone(await this.bindingFixtures.review(structuredClone(query))); }
  async associationProgress(query: Contracts.AssociationProgressQuery) { if (!this.bindingFixtures.progress) throw new Error('受控后端尚未安装 Rust 接入阶段投影。'); return structuredClone(await this.bindingFixtures.progress(structuredClone(query))); }
  async sourceSelection(query: Contracts.SourceSelectionQuery) { if (!this.bindingFixtures.sources) throw new Error('受控后端尚未安装 Rust 正文来源投影。'); return structuredClone(await this.bindingFixtures.sources(structuredClone(query))); }
  async beginReading(request: Contracts.BeginReadingRequest) { if (!this.bindingFixtures.begin) throw new Error('受控后端尚未安装真实选择提交。'); return structuredClone(await this.bindingFixtures.begin(structuredClone(request))); }
  async retryReadingWindowEffects(authority: Contracts.ReadingWindowEffectAuthority) { if (!this.bindingFixtures.effects) throw new Error('受控后端尚未安装真实窗口效果重试。'); return structuredClone(await this.bindingFixtures.effects(structuredClone(authority))); }
  serviceFormFixture: ((request: Contracts.ServiceFormRequest) => Promise<Contracts.ServiceFormProjection>) | null = null;
  async serviceForm(request: Contracts.ServiceFormRequest): Promise<Contracts.ServiceFormProjection> {
    if (!this.serviceFormFixture) throw new Error('Controlled Rust service-form projections are not installed for this test.');
    return structuredClone(await this.serviceFormFixture(structuredClone(request)));
  }
  catalogFixture: Contracts.ServiceCatalog | null = null;
  async serviceCatalog(): Promise<Contracts.ServiceCatalog> {
    if (!this.catalogFixture) throw new Error('Controlled service catalogue is not installed for this test.');
    return structuredClone(this.catalogFixture);
  }
  serviceFixtures: {
    configuration?: () => Promise<Contracts.ServiceConfigurationSnapshot>;
    draft?: (query: Contracts.ServiceDraftQuery) => Promise<Contracts.ServiceDraftEditorProjection>;
    cancelTest?: (operationId: string, profileGeneration: number) => Promise<void>;
    test?: (input: Contracts.SavedServiceTestRequest, operationId?: string, selectedTokenIndex?: number) => Promise<Contracts.ServiceTestResult>;
    save?: (input: Contracts.SaveServiceDraftInput) => Promise<Contracts.ServiceDraftReceipt>;
    saved?: (query: Contracts.SavedServiceQuery) => Promise<Contracts.SavedServiceProjection>;
    select?: (input: Contracts.SelectSavedServicesInput) => Promise<Contracts.ServiceSelectionReceipt>;
    enabled?: (input: Contracts.SetServicePurposeEnabledInput) => Promise<Contracts.ServiceSelectionReceipt>;
    inherit?: (input: Contracts.RestoreServiceInheritanceInput) => Promise<Contracts.ServiceSelectionReceipt>;
    credential?: (input: Contracts.ServiceCredentialRequest) => Promise<Contracts.CredentialReference>;
  } = {};
  async serviceDraft(query: Contracts.ServiceDraftQuery) { if (!this.serviceFixtures.draft) throw new Error('Controlled service-editor projection is not installed.'); return structuredClone(await this.serviceFixtures.draft(structuredClone(query))); }
  async testSavedService(input: Contracts.SavedServiceTestRequest, operationId?: string, selectedTokenIndex?: number) { if (!this.serviceFixtures.test) throw new Error('Controlled saved-service test handler is not installed.'); return structuredClone(await this.serviceFixtures.test(structuredClone(input), operationId, selectedTokenIndex)); }
  async cancelSavedServiceTest(operationId: string, generation: number) { if (!this.serviceFixtures.cancelTest) throw new Error('Controlled test cancellation is not installed.'); await this.serviceFixtures.cancelTest(operationId, generation); }
  async serviceConfiguration(): Promise<Contracts.ServiceConfigurationSnapshot> { if (this.serviceFixtures.configuration) return structuredClone(await this.serviceFixtures.configuration()); return { ...structuredClone(this.state.configuration), profileGeneration: this.state.profileGeneration }; }
  async saveServiceDraft(input: Contracts.SaveServiceDraftInput) { if (!this.serviceFixtures.save) throw new Error('Controlled service-save handler is not installed.'); return structuredClone(await this.serviceFixtures.save(structuredClone(input))); }
  async savedServices(query: Contracts.SavedServiceQuery) { if (!this.serviceFixtures.saved) throw new Error('Controlled saved-service projection is not installed.'); return structuredClone(await this.serviceFixtures.saved(structuredClone(query))); }
  async selectSavedServices(input: Contracts.SelectSavedServicesInput) { if (!this.serviceFixtures.select) throw new Error('Controlled selection mutation is not installed.'); return structuredClone(await this.serviceFixtures.select(structuredClone(input))); }
  async setServicePurposeEnabled(input: Contracts.SetServicePurposeEnabledInput) { if (!this.serviceFixtures.enabled) throw new Error('Controlled purpose mutation is not installed.'); return structuredClone(await this.serviceFixtures.enabled(structuredClone(input))); }
  async restoreServiceInheritance(input: Contracts.RestoreServiceInheritanceInput) { if (!this.serviceFixtures.inherit) throw new Error('Controlled inheritance mutation is not installed.'); return structuredClone(await this.serviceFixtures.inherit(structuredClone(input))); }
  async storeServiceCredential(input: Contracts.ServiceCredentialRequest) { if (!this.serviceFixtures.credential) throw new Error('Controlled typed credential handler is not installed.'); return this.serviceFixtures.credential(input); }
  googleAuthorizationFixtures: {
    inspect?: (context: Contracts.GoogleInteractiveContext) => Promise<Contracts.GoogleAuthorizationProjection>;
    start?: (input: Contracts.StartGoogleLoginInput) => Promise<Contracts.GoogleLoginOperationProjection>;
    status?: (query: Contracts.GoogleLoginOperationQuery) => Promise<Contracts.GoogleLoginOperationProjection>;
    cancel?: (query: Contracts.GoogleLoginOperationQuery) => Promise<Contracts.GoogleLoginOperationProjection>;
    revoke?: (input: Contracts.RevokeGoogleAuthorizationInput) => Promise<Contracts.GoogleRevokeProjection>;
  } = {};
  async inspectGoogleAuthorization(context: Contracts.GoogleInteractiveContext): Promise<Contracts.GoogleAuthorizationProjection> {
    if (!this.googleAuthorizationFixtures.inspect) return { state: 'unconfigured', canLogin: false, canLogout: false, message: '受控界面没有安装原生 Google 授权；本页不能登录。', affectedServiceCount: 0 };
    return structuredClone(await this.googleAuthorizationFixtures.inspect(structuredClone(context)));
  }
  async startGoogleLogin(input: Contracts.StartGoogleLoginInput) { if (!this.googleAuthorizationFixtures.start) throw new Error('Controlled native Google login is not installed.'); return structuredClone(await this.googleAuthorizationFixtures.start(structuredClone(input))); }
  async googleLoginStatus(query: Contracts.GoogleLoginOperationQuery) { if (!this.googleAuthorizationFixtures.status) throw new Error('Controlled native Google status is not installed.'); return structuredClone(await this.googleAuthorizationFixtures.status(structuredClone(query))); }
  async cancelGoogleLogin(query: Contracts.GoogleLoginOperationQuery) { if (!this.googleAuthorizationFixtures.cancel) throw new Error('Controlled native Google cancellation is not installed.'); return structuredClone(await this.googleAuthorizationFixtures.cancel(structuredClone(query))); }
  async revokeGoogleAuthorization(input: Contracts.RevokeGoogleAuthorizationInput) { if (!this.googleAuthorizationFixtures.revoke) throw new Error('Controlled native Google revocation is not installed.'); return structuredClone(await this.googleAuthorizationFixtures.revoke(structuredClone(input))); }
  state = emptySnapshot();
  sessionRecordFixtures: Record<string, Contracts.SessionRecord[]> = {};
  sessionCompletenessFixtures: Record<string, Contracts.SessionCompleteness> = {};
  sessionRecordQueries: Contracts.SessionRecordQuery[] = [];
  sessionRecordCursorSequence = 0;
  sessionRecordCursors = new Map<string, { signature: string; highWatermark: string; offset: number; snapshotId: string }>();
  sessionExportPlans = new Map<string, Contracts.SessionExportPlan>();
  sessionExportActions: { action: string; exportId: string }[] = [];
  sessionRecordHoldSearch: string[] = [];
  sessionRecordHolds = new Map<string, () => void>();
  failNextSessionRecords = false;
  cancelNextSessionExport = false;
  async sessionRecords(query: Contracts.SessionRecordQuery): Promise<Contracts.SessionRecordPage> {
    this.sessionRecordQueries.push(structuredClone(query));
    if (query.filter.search && this.sessionRecordHoldSearch.includes(query.filter.search)) { await new Promise<void>(resolve => this.sessionRecordHolds.set(query.filter.search!, resolve)); }
    if (this.failNextSessionRecords) { this.failNextSessionRecords = false; throw new Error('Controlled session page load failed'); }
    if (query.pageSize < 1 || query.pageSize > 200) throw new Error('Controlled page size invalid');
    const signature = JSON.stringify([query.sessionId, query.dataset, query.filter, query.order ?? 'oldest_first']);
    const fixture = this.sessionRecordFixtures[`${query.sessionId}:${query.dataset}`];
    const all: Contracts.SessionRecord[] = fixture ?? (query.dataset === 'body' ? this.state.history.units.filter(unit => unit.sessionId === query.sessionId).map((unit, index) => ({ kind: 'body', sequence: String(index + 1), unit: structuredClone(unit), sourceRefs: [] })) : []);
    const filtered = all.filter(record => {
      const text = record.kind === 'body' ? record.unit.sourceText : record.kind === 'extraction' ? record.text ?? '' : record.message;
      return (!query.filter.search || text.includes(query.filter.search) || record.kind === 'hook_event' && record.code.includes(query.filter.search)) && (!query.filter.sourceId || record.kind === 'extraction' && record.sourceId === query.filter.sourceId) && (!query.filter.level || record.kind === 'hook_event' && record.level === query.filter.level) && (!query.filter.stage || record.kind === 'hook_event' && (query.filter.stage === 'diagnostics' ? !(record.level === 'info' && record.stage === 'capture' && record.integrity === 'intact' && record.lostCount === null && ['text_unselected', 'text_paused', 'text_empty', 'text_inactive'].includes(record.code)) : record.stage === query.filter.stage));
    }).sort((a, b) => BigInt(a.sequence) < BigInt(b.sequence) ? -1 : BigInt(a.sequence) > BigInt(b.sequence) ? 1 : 0);
    let scope = query.cursor ? this.sessionRecordCursors.get(query.cursor) : undefined;
    if (query.cursor && (!scope || scope.signature !== signature)) throw new Error('Controlled cursor does not belong to this query');
    if (!scope) scope = { signature, highWatermark: all.at(-1)?.sequence ?? '0', offset: 0, snapshotId: `ui-scope-${++this.sessionRecordCursorSequence}` };
    const frozen = filtered.filter(record => BigInt(record.sequence) <= BigInt(scope!.highWatermark));
    const records = query.order === 'newest_first' ? frozen.slice(Math.max(0, frozen.length - scope.offset - query.pageSize), frozen.length - scope.offset) : frozen.slice(scope.offset, scope.offset + query.pageSize);
    const register = (offset: number) => { const cursor = `ui-opaque-${++this.sessionRecordCursorSequence}`; this.sessionRecordCursors.set(cursor, { ...scope!, offset }); return cursor; };
    const snapshot: Contracts.SessionRangeSnapshot = { snapshotId: scope.snapshotId, refreshCursor: register(0), highWatermark: scope.highWatermark, firstAvailable: all[0]?.sequence ?? null, lastReceived: all.at(-1)?.sequence ?? null, lastPersisted: all.at(-1)?.sequence ?? null, retainedFrom: all[0]?.sequence ?? null, matchingCount: String(frozen.length), newerCount: String(filtered.filter(record => BigInt(record.sequence) > BigInt(scope!.highWatermark)).length), databaseRevision: String(this.state.revision) };
    return { records: structuredClone(records), nextCursor: scope.offset + records.length < frozen.length ? register(scope.offset + records.length) : null, snapshot, loadedRange: { first: records[0]?.sequence ?? null, last: records.at(-1)?.sequence ?? null }, completeness: this.sessionCompletenessFixtures[`${query.sessionId}:${query.dataset}`] ?? { status: 'complete_received_range', scope: 'received_range_only', knownLoss: false, unknownRange: false, cleanedPayloadCount: '0', notes: ['Controlled UI collection; no actual producer or SQLite execution.'] } };
  }
  async prepareSessionExport(request: Contracts.SessionExportRequest): Promise<Contracts.SessionExportPlan> {
    if (!request.privacyConfirmed) throw new Error('Controlled export privacy acknowledgement required');
    const page = await this.sessionRecords(request.query);
    const plan: Contracts.SessionExportPlan = { exportId: `ui-export-${this.sessionExportPlans.size + 1}`, sessionId: request.query.sessionId, dataset: request.query.dataset, order: 'oldest_first', filter: structuredClone(request.query.filter), snapshot: page.snapshot, completeness: page.completeness, containsGameText: request.query.dataset !== 'hook_events', containsTargetIdentity: request.query.dataset !== 'body' };
    this.sessionExportPlans.set(plan.exportId, plan); this.sessionExportActions.push({ action: 'prepare', exportId: plan.exportId }); return structuredClone(plan);
  }
  async confirmSessionExport(exportId: string): Promise<Contracts.SessionExportReceipt> {
    const plan = this.sessionExportPlans.get(exportId); if (!plan) throw new Error('Controlled export plan missing');
    this.sessionExportActions.push({ action: 'confirm', exportId }); this.sessionExportPlans.delete(exportId);
    const cancelled = this.cancelNextSessionExport; this.cancelNextSessionExport = false;
    return { exportId, status: cancelled ? 'cancelled' : 'completed', recordCount: cancelled ? '0' : plan.snapshot.matchingCount, destinationLabel: cancelled ? null : 'controlled-ui-export.json', contentSha256: null };
  }
  async cancelSessionExport(exportId: string) { this.sessionExportActions.push({ action: 'cancel', exportId }); this.sessionExportPlans.delete(exportId); }
  readonly controlledPlatformEvidence = 'ui_fixture_not_native_runtime';
  // Explicit transport fixtures only. No ServiceManagement calls or native proof.
  loginStartupFixture: Contracts.LoginStartupState = { status: 'unsupported', canChange: false, errorCode: null, detail: 'Controlled UI transport has no native login registration.' };
  loginStartupResponses: { value?: Contracts.LoginStartupState; error?: string; holdKey?: string }[] = [];
  loginStartupSetResponses: { value?: Contracts.LoginStartupState; error?: string; holdKey?: string }[] = [];
  loginStartupQueries = 0;
  loginStartupRequests: boolean[] = [];
  loginStartupPending = new Map<string, () => void>();
  releaseLoginStartup(key: string) { const resolve = this.loginStartupPending.get(key); if (!resolve) throw new Error('Controlled login startup hold missing'); this.loginStartupPending.delete(key); resolve(); }
  async loginStartup(): Promise<Contracts.LoginStartupState> {
    this.loginStartupQueries += 1;
    const response = this.loginStartupResponses.shift();
    const value = structuredClone(response?.value ?? this.loginStartupFixture);
    if (response?.holdKey) await new Promise<void>(resolve => this.loginStartupPending.set(response.holdKey!, resolve));
    if (response?.error) throw new Error(response.error);
    return value;
  }
  async setLoginStartup(enabled: boolean): Promise<Contracts.LoginStartupState> {
    this.loginStartupRequests.push(enabled);
    const response = this.loginStartupSetResponses.shift();
    if (!response) throw new Error('Controlled login startup write has no prepared response');
    // Complete a prepared system effect before delaying its transport reply.
    if (!response.error && response.value) this.loginStartupFixture = structuredClone(response.value);
    if (response.holdKey) await new Promise<void>(resolve => this.loginStartupPending.set(response.holdKey!, resolve));
    if (response.error) throw new Error(response.error);
    if (!response.value) throw new Error('Controlled login startup write has no prepared state');
    return structuredClone(response.value);
  }
  capturePauseResponses: { error?: string; holdKey?: string }[] = [];
  capturePausePending = new Map<string, () => void>();
  releaseCapturePause(key: string) { const resolve = this.capturePausePending.get(key); if (!resolve) throw new Error('Controlled pause hold missing'); this.capturePausePending.delete(key); resolve(); }
  nativeDragRequests = 0;
  hudFitRequests: number[] = [];
  async fitHudContent(height: number) { this.hudFitRequests.push(height); this.capabilityFixture.geometry.logicalBounds!.height = Math.max(140,Math.min(366,height)); return structuredClone(this.capabilityFixture.geometry); }
  copiedReadingUnits: { unitId: string; textRevision: number; text: string }[] = [];
  lexicalRetries: { unitId: string; textRevision: number }[] = [];
  previewGeneration = 0n;
  previewRequests: Contracts.PreviewStartRequest[] = [];
  previewListeners = new Map<symbol, { projection: Contracts.PreviewProjection; listener: (update: Contracts.PreviewUpdate) => void }>();
  previewProjections = new Map<string, Contracts.PreviewProjection>();
  capabilityFixture: Contracts.WindowCapabilities = {
    geometry: { state: 'restored', logicalBounds: { x: 0, y: 0, width: 560, height: 290 }, monitorId: 'controlled-monitor', scaleFactor: 1, errorCode: null },
    material: { requested: 'material', effective: 'native_material', systemReduceTransparency: false, effectiveReduceTransparency: false, errorCode: null },
    shortcuts: { state: 'active', registrationGeneration: '1', bindings: [], dispatchSuppressed: false },
  };
  capabilityResponses: { value?: Contracts.WindowCapabilities; error?: string; holdKey?: string }[] = [];
  capabilityQueries = 0;
  capabilityPending = new Map<string, () => void>();
  releaseCapability(key: string) { const resolve=this.capabilityPending.get(key);if(!resolve)throw new Error('Controlled capability hold missing');this.capabilityPending.delete(key);resolve(); }
  async windowCapabilities() {
    this.capabilityQueries += 1; const response=this.capabilityResponses.shift();const value=structuredClone(response?.value ?? this.capabilityFixture);
    if(response?.holdKey)await new Promise<void>(resolve=>this.capabilityPending.set(response.holdKey!,resolve));
    if(response?.error)throw new Error(response.error);return value;
  }
  // Explicit test-only scope projections. Rust/SQLite tests own actual merge,
  // transactions and atomic persistence; this transport never claims DB proof.
  scopedPreferenceSaves = new Map<string, { saved: Contracts.Preferences; headRevision: number; preferenceRevisionId: string }>();
  scopedPreferenceResponses: { scopeKey?: string; value?: Contracts.ScopedPreferences; error?: string; holdKey?: string }[] = [];
  scopedPreferenceQueries: string[] = [];
  scopedPreferencePending = new Map<string, () => void>();
  scopedPreferenceSaveResponses: { error?:string; capabilities?:Contracts.WindowCapabilities }[] = [];
  preferenceSaveHoldKeys: string[] = [];
  preferenceSavePending = new Map<string, () => void>();
  releasePreferenceScope(key: string) { const resolve=this.scopedPreferencePending.get(key);if(!resolve)throw new Error('Controlled scope hold missing');this.scopedPreferencePending.delete(key);resolve(); }
  releasePreferenceSave(key: string) { const resolve=this.preferenceSavePending.get(key);if(!resolve)throw new Error('Controlled preference save hold missing');this.preferenceSavePending.delete(key);resolve(); }
  controlledScope(scopeKey: string): Contracts.ScopedPreferences {
    if(scopeKey !== 'global' && (!scopeKey.startsWith('game:') || !this.state.games.some(game=>`game:${game.gameId}`===scopeKey)))throw new Error('Controlled preference scope game missing');
    const global=structuredClone(this.scopedPreferenceSaves.get('global')?.saved ?? (this.state.preferences.scopeKey==='global'?this.state.preferences:emptySnapshot().preferences));
    global.scopeKey='global';const local=this.scopedPreferenceSaves.get(scopeKey);const saved=local?.saved ?? null;const effective=structuredClone(global);const overriddenFields:Contracts.PreferenceField[]=[];
    if(scopeKey!=='global'&&saved){effective.scopeKey=scopeKey;for(const field of ['theme','transparencyMode','showRuby','showPos','sourceFontPt','translationFontPt','hudWidthPt','shortcuts','segmentationEnabled','readingDisplayMode','hudBackgroundOpacity'] as const){if(saved.overrides[field]===true||field==='segmentationEnabled'&&Object.hasOwn(saved.overrides,'reading.segmentationEnabled')){Object.assign(effective,{[field]:structuredClone(saved[field])});overriddenFields.push(field);}}if(Object.hasOwn(saved.overrides,'hud.compact'))overriddenFields.push('hudCompact');if(Object.hasOwn(saved.overrides,'hud.alwaysOnTop'))overriddenFields.push('hudAlwaysOnTop');effective.overrides={...global.overrides,...saved.overrides};}
    const inheritedDraft=scopeKey==='global'?null:{...structuredClone(saved ?? effective),scopeKey,overrides:{}};
    return{scopeKey,saved:structuredClone(saved),effective,global,headRevision:local?.headRevision??null,preferenceRevisionId:local?.preferenceRevisionId??null,overriddenFields,inheritedDraft};
  }
  async preferencesScope(scopeKey: string): Promise<Contracts.ScopedPreferences> {
    this.scopedPreferenceQueries.push(scopeKey);const index=this.scopedPreferenceResponses.findIndex(response=>!response.scopeKey||response.scopeKey===scopeKey);const response=index<0?undefined:this.scopedPreferenceResponses.splice(index,1)[0];const value=structuredClone(response?.value ?? this.controlledScope(scopeKey));
    if(response?.holdKey)await new Promise<void>(resolve=>this.scopedPreferencePending.set(response.holdKey!,resolve));if(response?.error)throw new Error(response.error);return value;
  }
  bindingFixture: Contracts.GameWindowBindingProjection = {state:'unbound',sessionId:null,targetIdentitySha256:null,windowTitle:null,verifiedAtMs:null,errorCode:null,detail:'Controlled UI fixture has no verified window binding.'};
  bindingQueries = 0;
  bindingResponses: { value?: Contracts.GameWindowBindingProjection; error?: string | Contracts.AppError; holdKey?: string }[] = [];
  bindingSettled = 0;
  bindingRejected: unknown[] = [];
  bindingPending = new Map<string, () => void>();
  activationResponses: { value?: Contracts.GameWindowActivationReceipt; error?: string; holdKey?:string }[] = [];
  activations: Contracts.GameWindowBindingProjection[] = [];
  activationRequests: {expectedSessionId:string;expectedTargetIdentitySha256:string}[] = [];
  unbindResponses: {holdKey?:string;error?:string}[] = [];
  unbindPending = new Map<string, () => void>();
  releaseUnbind(key:string) {const resolve=this.unbindPending.get(key);if(!resolve)throw new Error('Controlled unbind hold missing');this.unbindPending.delete(key);resolve();}
  activationPending = new Map<string, () => void>();
  releaseActivation(key:string) {const resolve=this.activationPending.get(key);if(!resolve)throw new Error('Controlled activation hold missing');this.activationPending.delete(key);resolve();}
  releaseBinding(key: string) {const resolve=this.bindingPending.get(key);if(!resolve)throw new Error('Controlled binding hold missing');this.bindingPending.delete(key);resolve();}
  async gameWindowBinding(): Promise<Contracts.GameWindowBindingProjection> {
    this.bindingQueries+=1;const response=this.bindingResponses.shift(),value=structuredClone(response?.value??this.bindingFixture);
    try { if(response?.holdKey)await new Promise<void>(resolve=>this.bindingPending.set(response.holdKey!,resolve));if(response?.error){this.bindingRejected.push(structuredClone(response.error));throw typeof response.error==='string'?new Error(response.error):structuredClone(response.error);}return value; }
    finally { this.bindingSettled+=1; }
  }
  async showAssociatedGame(request:{expectedSessionId:string;expectedTargetIdentitySha256:string}):Promise<Contracts.GameWindowActivationReceipt> {
    this.activationRequests.push(structuredClone(request));
    const assertExpected=()=>{if(this.bindingFixture.state!=='bound'||this.bindingFixture.sessionId!==request.expectedSessionId||this.bindingFixture.targetIdentitySha256!==request.expectedTargetIdentitySha256)throw new Error('Controlled activation expected target changed');};
    assertExpected();const response=structuredClone(this.activationResponses.shift());
    if(response?.holdKey)await new Promise<void>(resolve=>this.activationPending.set(response.holdKey!,resolve));
    assertExpected();if(response?.error)throw new Error(response.error);if(!response?.value)throw new Error('Controlled activation response has not been prepared');
    this.activations.push(structuredClone(this.bindingFixture));return response.value;
  }

  async beginHudDrag() { this.nativeDragRequests += 1; }
  async resetHudPosition() { this.capabilityFixture.geometry.state = 'restored'; return structuredClone(this.capabilityFixture.geometry); }
  async setShortcutRecording(recording: boolean) { this.capabilityFixture.shortcuts.dispatchSuppressed = recording; return structuredClone(this.capabilityFixture.shortcuts); }
  coverFixtures: Record<string, Contracts.GameCoverResult> = {};
  coverRequests: Contracts.GameCoverRequest[] = [];
  previewDetaches: string[] = [];
  previewControls: Contracts.PreviewControlRequest[] = [];
  previewControlHolds: { action: Contracts.PreviewControlRequest['action']; key: string }[] = [];
  previewControlPending = new Map<string, () => void>();
  releasePreviewControl(key: string) { const resume = this.previewControlPending.get(key); if (!resume) throw new Error('Controlled preview response is not pending'); this.previewControlPending.delete(key); resume(); }
  holdAssociation = false;
  associationResume: (() => void) | null = null;
  previewSessionQueries: string[] = [];
  previewAttaches: string[] = [];
  previewSubscriptionHistory: { id: number; projection: Contracts.PreviewProjection; unsubscribe: () => void }[] = [];
  // Explicit controlled latest-frame responses, retained across view detaches.
  // The production adapter obtains these bytes from preview_latest, not a new capture.
  previewFrameFixtures = new Map<string, Contracts.PreviewUpdate['frame']>();
  previewCapabilityFixture: Contracts.PreviewCapability = { supported: true, permission: 'granted', errorCode: null, detail: 'Controlled UI permission transport; not an OS permission result.' };
  // Explicit component-host owner input, installed by the pre-render hook.
  // This is controlled transport data, never a production bootstrap option.
  reviewedCandidatePreviewFixture: { candidateId: string; projection: Contracts.PreviewProjection } | null = null;
  previewPickerRequests: Contracts.PreviewStartRequest[] = [];
  // Explicit controlled replies only; this adapter never opens a native picker.
  previewPickerFixtures: { projection?: Omit<Contracts.PreviewProjection, 'consumerId'>; error?: string; holdKey?: string }[] = [];
  previewPickerPending = new Map<string, () => void>();
  releasePreviewPicker(key: string) {
    const resolve = this.previewPickerPending.get(key);
    if (!resolve) throw new Error('Controlled window picker response is not pending');
    this.previewPickerPending.delete(key); resolve();
  }
  async readGameCover(request: Contracts.GameCoverRequest): Promise<Contracts.GameCoverResult> {
    this.coverRequests.push(structuredClone(request));
    const result = this.coverFixtures[request.gameId];
    return structuredClone(result ?? { gameId: request.gameId, assetRef: null, status: 'missing', origin: null, contentHash: null, width: null, height: null, mimeType: null, payload: null, errorCode: null });
  }
  async previewSession(): Promise<Contracts.PreviewProjection> {
    const sessionId = this.state.session?.sessionId ?? '';
    this.previewSessionQueries.push(sessionId);
    const projection = [...this.previewProjections.values()].find(value => value.consumerId === sessionId && value.state !== 'stopped');
    return structuredClone(projection ?? { leaseId: null, generation: '0', consumerId: sessionId, state: 'unavailable', permission: 'not_requested', targetIdentityHash: null, attemptedAtMs: null, capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: null, errorCode: 'controlled_session_preview_unavailable', detail: 'Controlled UI transport has no prepared session preview.' });
  }
  attachWindowPreview(projection: Contracts.PreviewProjection, listener: (update: Contracts.PreviewUpdate) => void): () => void {
    if (!projection.leaseId) return () => {};
    if ([...this.previewListeners.values()].filter(item => item.projection.leaseId === projection.leaseId).length >= 4) throw new Error('同一窗口最多支持四个同时预览的订阅。');
    const token = Symbol('controlled-preview-subscriber');
    this.previewAttaches.push(projection.leaseId); this.previewListeners.set(token, { projection, listener });
    const frame = this.previewFrameFixtures.get(`${projection.leaseId}:${projection.generation}`);
    if (frame && frame.targetIdentityHash === projection.targetIdentityHash) queueMicrotask(() => { if (this.previewListeners.has(token)) listener(structuredClone({ projection, frame })); });
    let active = true;
    const unsubscribe = () => { if (!active) return; active = false; this.previewListeners.delete(token); this.previewDetaches.push(projection.leaseId!); };
    this.previewSubscriptionHistory.push({ id: this.previewSubscriptionHistory.length + 1, projection: structuredClone(projection), unsubscribe });
    return unsubscribe;
  }
  async previewCapability(): Promise<Contracts.PreviewCapability> { return structuredClone(this.previewCapabilityFixture); }
  /** Explicit Rust-session transport setup; no screen capture or lifecycle proof. */
  prepareSessionPreview(sessionId: string, targetIdentityHash: string, state: Contracts.PreviewProjection['state'] = 'starting'): Contracts.PreviewProjection {
    for (const previous of this.previewProjections.values()) if (previous.consumerId === sessionId) previous.state = 'stopped';
    const generation = String(++this.previewGeneration);
    const projection: Contracts.PreviewProjection = { leaseId: `ui-session-lease-${generation}`, generation, consumerId: sessionId, state, permission: 'granted', targetIdentityHash, attemptedAtMs: '1792368000000', capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: '1792368000000', errorCode: null, detail: 'Explicit controlled Rust session-lease projection; no actual capture occurred.' };
    this.previewProjections.set(projection.leaseId!, projection); return structuredClone(projection);
  }
  async startWindowPreview(request: Contracts.PreviewStartRequest): Promise<Contracts.PreviewProjection> {
    this.previewRequests.push(structuredClone(request));
    const projection: Contracts.PreviewProjection = { leaseId: `ui-lease-${this.previewRequests.length}`, generation: String(++this.previewGeneration), consumerId: request.consumerId, state: 'starting', permission: 'granted', targetIdentityHash: 'f'.repeat(64), attemptedAtMs: '1792368000000', capturedAtMs: null, contentChangedAtMs: null, permissionCheckedAtMs: '1792368000000', errorCode: null, detail: 'Controlled UI frame lease; not a screen-capture result.' };
    this.previewProjections.set(projection.leaseId!, projection); return structuredClone(projection);
  }
  async selectWindowPreview(request: Contracts.PreviewStartRequest): Promise<Contracts.PreviewProjection> {
    this.previewPickerRequests.push(structuredClone(request));
    const fixture = this.previewPickerFixtures.shift();
    if (!fixture || (fixture.projection === undefined) === (fixture.error === undefined)) throw new Error('Exactly one explicit controlled picker reply is required');
    const projection = fixture.projection ? { ...structuredClone(fixture.projection), consumerId: request.consumerId } : null;
    if (projection) {
      for (const previous of this.previewProjections.values()) if (previous.consumerId === request.consumerId && previous.leaseId !== projection.leaseId) previous.state = 'stopped';
      if (projection.leaseId) this.previewProjections.set(projection.leaseId, projection);
    }
    // The native effect can finish before an IPC reply reaches the old view.
    if (fixture.holdKey) await new Promise<void>(resolve => this.previewPickerPending.set(fixture.holdKey!, resolve));
    if (fixture.error !== undefined) throw new Error(fixture.error);
    return structuredClone(projection!);
  }
  emitPreview(update: Contracts.PreviewUpdate) {
    const previous = this.previewProjections.get(update.projection.leaseId!);
    // A delivered stale transport callback must not rewrite the prepared
    // authoritative server projection that previewSession returns later.
    if (!previous || BigInt(update.projection.generation) >= BigInt(previous.generation)) this.previewProjections.set(update.projection.leaseId!, structuredClone(update.projection));
    const frame = update.frame;
    if (frame && frame.leaseId === update.projection.leaseId && frame.generation === update.projection.generation && frame.targetIdentityHash === update.projection.targetIdentityHash) {
      const key = `${frame.leaseId}:${frame.generation}`, retained = this.previewFrameFixtures.get(key);
      if (!retained || BigInt(frame.sequence) > BigInt(retained.sequence)) this.previewFrameFixtures.set(key, structuredClone(frame));
    }
    for (const {projection,listener} of [...this.previewListeners.values()]) if (projection.consumerId === update.projection.consumerId && projection.leaseId === update.projection.leaseId && projection.generation === update.projection.generation) listener(structuredClone(update));
  }
  async controlWindowPreview(request: Contracts.PreviewControlRequest): Promise<Contracts.PreviewProjection> {
    this.previewControls.push(structuredClone(request));
    const previous = this.previewProjections.get(request.leaseId); if (!previous || previous.generation !== request.generation) throw new Error('Controlled preview lease is stale');
    const projection = { ...previous, errorCode: request.action === 'resume' && previous.errorCode !== 'preview_window_selection_required' ? null : previous.errorCode, generation: String(++this.previewGeneration), state: request.action === 'stop' ? 'stopped' as const : request.action === 'pause' ? 'paused' as const : previous.errorCode === 'preview_window_selection_required' ? 'unavailable' as const : 'starting' as const };
    // Controlled response for the service's paused latest-frame contract.
    // Actual retention and stale-callback rejection are checked in Rust tests.
    const retained = this.previewFrameFixtures.get(`${previous.leaseId}:${previous.generation}`);
    if (request.action === 'pause' && retained) this.previewFrameFixtures.set(`${projection.leaseId}:${projection.generation}`, { ...structuredClone(retained), generation: projection.generation });
    this.previewProjections.set(request.leaseId, projection);
    const hold = this.previewControlHolds.findIndex(value => value.action === request.action);
    if (hold >= 0) { const {key} = this.previewControlHolds.splice(hold, 1)[0]; await new Promise<void>(resolve => this.previewControlPending.set(key, resolve)); }
    return structuredClone(projection);
  }
  async copyReadingUnit(unitId: string, textRevision: number) { const unit = await this.readingUnit(unitId, textRevision); this.copiedReadingUnits.push({ unitId, textRevision, text: unit.sourceText }); }
  async retryLexical(unitId: string, textRevision: number) { const unit = await this.readingUnit(unitId, textRevision); this.lexicalRetries.push({ unitId, textRevision }); this.publish({ lexicalStates: [...this.state.lexicalStates.filter(state => state.unitId !== unitId || state.textRevision !== textRevision), { unitId, textRevision, sourceHash: unit.sourceHash, state: 'pending', errorCode: null }] }); }
  async cycleReadingDisplayMode() { await this.dispatch({ operationId: crypto.randomUUID(), expectedRevision: this.state.commitRevision, command: { kind: 'cycle_reading_display_mode' } }); }
  saveGamesResponses: { games: Game[]; gameResults: GameSaveResult[]; error?: string }[] = [];
  saveGamesHolds: string[] = [];
  saveGamesPending = new Map<string, () => void>();
  releaseSaveGames(key: string) { const resume = this.saveGamesPending.get(key); if (!resume) throw new Error('Controlled save response is not pending'); this.saveGamesPending.delete(key); resume(); }
  targetRequests = 0;
  targetResponses: { value?: Contracts.TargetCandidates; error?: string; holdKey?: string }[] = [];
  targetPending = new Map<string, Set<() => void>>();
  releaseTargets(key: string) { const resumes = this.targetPending.get(key); if (!resumes) throw new Error('Controlled target response is not pending'); this.targetPending.delete(key); for (const resume of resumes) resume(); }
  associationResponses: { error?: string }[] = [];
  copyResponses: { configuration?: ConfigurationState; resultId?: string; error?: string }[] = [];
  wordCard: FrozenWordCard | null = null;
  wordCardRequests: OpenWordCardRequest[] = [];
  wordCardRect: OpenWordCardRequest['anchorRect'] | null = null;
  private wordCardSequence = 0;
  private wordCardListeners = new Set<() => void>();
  private wordCardClosedListeners = new Set<(cardId: string) => void>();
  subscribeWordCardSurface = (listener: () => void) => { this.wordCardListeners.add(listener); return () => { this.wordCardListeners.delete(listener); }; };
  getWordCardSurface = () => this.wordCard;

  commands: CommandEnvelope[] = [];
  windows: WindowRole[] = [];
  associations: AssociateRequest[] = [];
  mainPageRequests: import('../../src/bridge/contracts.generated').MainPageRequest[] = [];
  private mainPageListeners = new Set<(request: import('../../src/bridge/contracts.generated').MainPageRequest) => void>();
  async openMainPage(intent: import('../../src/bridge/contracts.generated').MainPageIntent) {
    const request = { revision: (this.mainPageRequests.at(-1)?.revision ?? 0) + 1, intent };
    this.mainPageRequests.push(request); this.windows.push('main');
    for (const listener of this.mainPageListeners) listener(request);
  }
  async onMainPageRequested(listener: (request: import('../../src/bridge/contracts.generated').MainPageRequest) => void) {
    this.mainPageListeners.add(listener);
    const pending = this.mainPageRequests.at(-1); if (pending) listener(pending);
    return () => { this.mainPageListeners.delete(listener); };
  }
  settingsPageRequests: Contracts.SettingsPageRequest[] = [];
  private settingsPageListeners = new Set<(request: Contracts.SettingsPageRequest) => void>();
  async openSettingsPage(intent: Contracts.SettingsPageIntent) {
    const request = { revision: (this.settingsPageRequests.at(-1)?.revision ?? 0) + 1, intent };
    this.settingsPageRequests.push(request); this.windows.push('settings');
    for (const listener of this.settingsPageListeners) listener(request);
  }
  async onSettingsPageRequested(listener: (request: Contracts.SettingsPageRequest) => void) {
    this.settingsPageListeners.add(listener);
    const pending = this.settingsPageRequests.at(-1); if (pending) listener(pending);
    return () => { this.settingsPageListeners.delete(listener); };
  }
  launches: { gameId: string; rowRevision: number }[] = [];
  launchResponses: { error?: string }[] = [];
  snapshots = 0;
  dictionaryEntries: DictionaryEntry[] = [];
  targetCandidates: TargetCandidate[] = [{ candidateId: 'ui-target', windowTitle: 'UI controlled window', executablePath: 'C:\\UI-test\\Game.exe', windowsPid: 321, processCreatedU64: '12345', architecture: 'x86', environmentKey: 'crossover:/UI-test/Bottle', bottleLabel: 'UI-test', hasWindow: true, windowHandle: null, macosHostPid: null, macosWindowId: null }];
  runtimeResponses: Partial<ConfigurationState>[] = [];
  credentialImpact: CredentialRevocationImpact | null = null;
  credentialImpactRequests: string[] = [];
  revocations: CredentialRevocationRequest[] = [];
  revocationResponses: { patch: Partial<AppSnapshot>; error: string | null }[] = [];
  failNextCommand = false;
  // Controlled responses describe transport outcomes, never production CAS logic.
  configurationTests: ConfigurationTestRequest[] = [];
  configurationTestResponses: { patch?: Partial<AppSnapshot>; error?: string; holdKey?: string }[] = [];
  configurationTestPending = new Map<string, () => void>();
  snapshotFailures: string[] = [];
  releaseConfigurationTest(key: string) { const resolve = this.configurationTestPending.get(key); if (!resolve) throw new Error('UI transport: missing held configuration test'); this.configurationTestPending.delete(key); resolve(); }

  executableSelections: ExecutableSelection[] = [{ path: '/UI-test/Game.exe', displayName: 'UI controlled game', architecture: 'x86', environmentKey: 'crossover:/UI-test/Bottle', bottleLocator: '/UI-test/Bottle' }];
  studies: { unitId: string; textRevision: number; purpose: string; tokenIndex: number | null }[] = [];
  historyPages: Record<string, HistoryPage> = {};
  historyQueries: HistoryQuery[] = [];
  historySessionQueries: HistorySessionsQuery[] = [];
  historyHolds: { kind: 'units' | 'sessions'; gameId: string | null; key: string }[] = [];
  historyPending = new Map<string, () => void>();
  historyReleased: string[] = [];
  releaseHistory(key: string) { const resolve = this.historyPending.get(key); if (!resolve) throw new Error('UI transport: missing held history response'); this.historyPending.delete(key); resolve(); }
  private async holdHistory(kind: 'units' | 'sessions', gameId: string | null) {
    const index = this.historyHolds.findIndex(hold => hold.kind === kind && hold.gameId === gameId);
    if (index < 0) return;
    const [hold] = this.historyHolds.splice(index, 1);
    await new Promise<void>(resolve => { this.historyPending.set(hold.key, resolve); }); this.historyReleased.push(hold.key);
  }
  private legacyHistoryCursorScopes = new Map<string, string>();
  private historySourceKind(unit: ReadingUnit): 'hook' | 'manual' { return unit.sourceSummary?.sourceKind ?? (this.state.session?.sessionId === unit.sessionId && this.state.session.sourceKind === 'hook' ? 'hook' : unit.gameId ? 'hook' : 'manual'); }
  private pageOffset(cursor: string | null, scope: string, limit: number) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 200) throw new Error('UI transport: invalid history page limit');
    if (!cursor) return 0;
    let value: { scope?: string; offset?: number };
    try { value = JSON.parse(cursor); } catch { throw new Error('UI transport: invalid history cursor'); }
    if (value.scope !== scope || !Number.isInteger(value.offset) || value.offset! < 0) throw new Error('UI transport: history cursor scope mismatch');
    return value.offset!;
  }
  private dropListeners = new Set<(paths: string[]) => void>();
  emitDrop(paths: string[]) { this.dropListeners.forEach(listener => listener(paths)); }
  private listeners = new Set<(update: ProjectionUpdate) => void>();
  constructor(scenario: string) {
    if (scenario === 'model-config') {
      this.state.configuration.draft = { kind: 'legacy', configuration: modelConfigFixture() };
      this.state.games = [{ gameId: 'ui-term-game', displayName: 'UI glossary scope', originalTitle: null, sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1, target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } }];
    }
    if (scenario === 'configuration-lifecycle') {
      const applied = modelConfigFixture(); applied.revisionId = 'ui-applied-lifecycle';
      applied.connections[0] = { ...applied.connections[0], displayName: 'UI applied shared service', authMode: 'api_key', credentialPoolId: 'ui-shared-pool', credentialPoolRevision: 2 };
      applied.bindings[0].enabled = true;
      applied.bindings.push({ ...structuredClone(applied.bindings[0]), bindingId: 'ui-binding-grammar', purpose: 'grammar', outputMode: 'plain_text', engines: [{ ...structuredClone(applied.bindings[0].engines[0]), engineId: 'ui-grammar-engine' }] });
      const impacts = applied.bindings.map(binding => ({ scopeKey: binding.scopeKey, purpose: binding.purpose, bindingId: binding.bindingId, enabledIntent: true, runtimeEnabled: true, affectedGameIds: ['ui-term-game'], engineIds: binding.engines.map(engine => engine.engineId) }));
      const draft = structuredClone(applied); draft.revisionId = 'ui-draft-lifecycle'; draft.connections[0].displayName = 'UI draft shared service';
      this.state.configuration = { draft: { kind: 'legacy', configuration: draft }, applied: { kind: 'legacy', configuration: applied }, draftHeadRevision: 4, appliedHeadRevision: 3, runtimeConnections: [{ connectionId: 'ui-connection-a', configRevisionId: applied.revisionId, controlRevision: 4, suspended: false, reasonCode: null, availabilityIntent: true, affectedPurposes: impacts }], runtimePurposes: impacts.map(({ engineIds, ...impact }, index) => ({ ...impact, configRevisionId: applied.revisionId, authorizationEpoch: 7 + index, blockedConnectionIds: [], authorizedEngineIds: engineIds })) };
      this.state.games = [{ gameId: 'ui-term-game', displayName: 'UI inherited game', originalTitle: null, sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1, target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } }];
      this.state.credentials = [{ credentialId: 'ui-credential-shared', maskedLabel: 'UI shared ·••1234', state: 'active', lifecycleRevision: 2 }];
      this.state.credentialPools = [{ poolId: 'ui-shared-pool', revision: 2, policy: 'selected', metadata: { label: 'UI shared pool' }, members: [{ memberId: 'ui-pool-member', credentialId: 'ui-credential-shared', position: 0, enabled: true, weight: 1 }], contentHash: 'c'.repeat(64) }];
      this.credentialImpact = { credentialId: 'ui-credential-shared', lifecycleRevision: 2, state: 'active', configRevisionId: applied.revisionId, impactHash: 'd'.repeat(64), affectedConnectionIds: ['ui-connection-a'], affectedPurposes: impacts };
    }
    // This named UI scenario explicitly declares a verified-binding transport
    // fixture. It is not inferred by the application or evidence of native binding.
    if (scenario === 'reading') { this.bindingFixture = { state: 'bound', sessionId: 'ui-fixture-session', targetIdentitySha256: 'b'.repeat(64), windowTitle: 'UI controlled window', verifiedAtMs: '1792368000000', errorCode: null, detail: 'Explicit controlled verified-binding projection; not a native attestation.' }; this.prepareSessionPreview('ui-fixture-session', 'f'.repeat(64)); }
    if (scenario === 'reading' || scenario === 'learning') { const unit = fixtureUnit(); unit.sourceSummary = { ...unit.sourceSummary!, sourceKind: 'hook', state: 'active', endedAtMs: null }; this.state.current = unit; this.state.history.units = [unit]; this.state.session = { sessionId: unit.sessionId, gameId: null, sourceKind: 'hook', state: 'active', paused: false, translationEnabled: true, sources: [{ sourceId: 'ui-stream', hookName: 'UI fixture text stream', hookCode: 'fixture', sample: unit.sourceText, selected: true, encoding: 'UTF-16' }] }; if (scenario === 'learning') this.state.learningObject = { unit: structuredClone(unit), tokenIndex: 0 }; }
  }
  publish(patch: Partial<AppSnapshot>, gap = false) { const old = this.state.revision; this.state = { ...this.state, ...structuredClone(patch), revision: old + (gap ? 2 : 1) }; const update: ProjectionUpdate = { projectionId: 'ui-test', serverInstanceId: this.state.serverInstanceId, profileGeneration: this.state.profileGeneration, baseRevision: gap ? old + 1 : old, revision: this.state.revision, snapshot: structuredClone(this.state) }; this.listeners.forEach(listener => listener(update)); }
  async snapshot() { this.snapshots++; const failure = this.snapshotFailures.shift(); if (failure) throw new Error(failure); return structuredClone(this.state); }
  async subscribe(listener: (update: ProjectionUpdate) => void) { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; }
  async dispatch(envelope: CommandEnvelope) {
    this.commands.push(structuredClone(envelope));
    if (this.failNextCommand) { this.failNextCommand = false; throw new Error('UI test transport: simulated save failure'); }
    const command = envelope.command;
    if(command.kind==='unbind_window') {
      if(this.bindingFixture.state!=='bound'||this.bindingFixture.sessionId!==command.payload.expectedSessionId||this.bindingFixture.targetIdentitySha256!==command.payload.expectedTargetIdentitySha256)throw new Error('Controlled unbind expected target changed');
      const response=this.unbindResponses.shift();
      // Complete the explicitly prepared target operation before delaying its
      // transport acknowledgement. A later session must not be mutated by it.
      if(!response?.error)this.bindingFixture={state:'unbound',sessionId:this.state.session?.sessionId??null,targetIdentitySha256:null,windowTitle:null,verifiedAtMs:'1792368000000',errorCode:null,detail:'Controlled unbind response; extraction remains active.'};
      if(response?.holdKey)await new Promise<void>(resolve=>this.unbindPending.set(response.holdKey!,resolve));
      if(response?.error)throw new Error(response.error);
    }
    let resultId: string | null = null; let gameResults: GameSaveResult[] | undefined;
    if (command.kind === 'save_game') { const gameId = command.payload.gameId ?? crypto.randomUUID(); const game = { ...command.payload, gameId, rowRevision: (command.payload.rowRevision ?? 0) + 1 }; this.publish({ games: [...this.state.games.filter(item => item.gameId !== gameId), game] }); resultId = gameId; }
    else if (command.kind === 'save_games') {
      // Only a controlled transport/projection. Rust tests own validation,
      // target reuse and transaction semantics; no domain lookup is re-created.
      const hold = this.saveGamesHolds.shift();
      if (hold) await new Promise<void>(resolve => this.saveGamesPending.set(hold, resolve));
      const response = this.saveGamesResponses.shift();
      if (response?.error) throw new Error(response.error);
      if (response) { gameResults = structuredClone(response.gameResults); this.publish({ games: structuredClone(response.games) }); }
      else {
        gameResults = command.payload.items.map(item => ({ itemId: item.itemId, gameId: crypto.randomUUID(), rowRevision: 1, reused: false }));
        const games = command.payload.items.map((item, index) => ({ ...structuredClone(item.game), gameId: gameResults![index].gameId, rowRevision: gameResults![index].rowRevision }));
        this.publish({ games: [...this.state.games, ...games] });
      }
    }
    else if (command.kind === 'archive_game') this.publish({ games: this.state.games.filter(game => game.gameId !== command.payload.gameId) });
    else if (command.kind === 'manual_input') { const unit = { ...fixtureUnit(), sourceText: command.payload.text, sourceLanguage: command.payload.sourceLanguage, tokens: [], unitId: crypto.randomUUID() }; resultId = unit.unitId; this.publish({ current: unit, history: { units: [unit, ...this.state.history.units], nextCursor: null } }); }
    else if (command.kind === 'copy_model_configuration') { const response = this.copyResponses.shift(); if (!response) throw new Error('UI transport has no prepared copy response'); if (response.error) throw new Error(response.error); if (!response.configuration) throw new Error('UI transport copy response has no configuration'); this.publish({ configuration: response.configuration }); resultId = response.resultId ?? null; }
    else if (command.kind === 'save_preferences') { const preferences=structuredClone(command.payload.preferences),previous=this.scopedPreferenceSaves.get(preferences.scopeKey);this.scopedPreferenceSaves.set(preferences.scopeKey,{saved:preferences,headRevision:(previous?.headRevision??0)+1,preferenceRevisionId:crypto.randomUUID()});this.publish({preferences}); }
    else if (command.kind === 'save_scoped_preferences') {
      const response=this.scopedPreferenceSaveResponses.shift();if(response?.capabilities)this.capabilityFixture=structuredClone(response.capabilities);if(response?.error)throw new Error(response.error);
      const hold=this.preferenceSaveHoldKeys.shift();if(hold)await new Promise<void>(resolve=>this.preferenceSavePending.set(hold,resolve));
      const preferences=structuredClone(command.payload.preferences);const scope=this.controlledScope(preferences.scopeKey);
      if(command.payload.expectedHeadRevision!==scope.headRevision)throw new Error('preference_head_conflict: Controlled scope head changed; saved projection remains unchanged');
      const preferenceRevisionId=crypto.randomUUID();this.scopedPreferenceSaves.set(preferences.scopeKey,{saved:preferences,headRevision:(scope.headRevision??0)+1,preferenceRevisionId});resultId=preferenceRevisionId;
      const activeGame=this.state.session?.gameId??this.state.current?.gameId;this.publish({preferences:this.controlledScope(activeGame?`game:${activeGame}`:'global').effective});
    }
    else if (command.kind === 'set_reading_annotations') this.publish({ preferences: { ...this.state.preferences, showRuby: command.payload.showRuby, showPos: command.payload.showPos } });
    else if (command.kind === 'cycle_reading_display_mode') { const modes = ['both', 'source', 'translation'] as const; this.publish({ preferences: { ...this.state.preferences, readingDisplayMode: modes[(modes.indexOf(this.state.preferences.readingDisplayMode) + 1) % modes.length] } }); }
    else if (command.kind === 'set_hud_preference') { const keys = { segmentation: 'reading.segmentationEnabled', compact: 'hud.compact', always_on_top: 'hud.alwaysOnTop' }; this.publish({ preferences: { ...this.state.preferences, ...(command.payload.preference === 'segmentation' ? { segmentationEnabled: command.payload.enabled } : {}), overrides: { ...this.state.preferences.overrides, [keys[command.payload.preference]]: command.payload.enabled } } }); }
    else if (command.kind === 'save_configuration') this.publish({ configuration: { ...this.state.configuration, draft: { kind: 'legacy', configuration: { ...command.payload.configuration, revisionId: crypto.randomUUID() } }, draftHeadRevision: this.state.configuration.draftHeadRevision + 1 } });
    else if (command.kind === 'apply_configuration') this.publish({ configuration: { ...this.state.configuration, applied: structuredClone(this.state.configuration.draft), appliedHeadRevision: this.state.configuration.appliedHeadRevision + 1 } });
    else if (command.kind === 'unbind_window') { /* Prepared transport acknowledgement above; no session mutation. */ }
    else if (command.kind === 'set_capture_paused') {
      const response = this.capturePauseResponses.shift();
      if (!this.state.session || this.state.session.state !== 'active') throw new Error('Controlled current session is not active');
      // The prepared command acts on this session now. Holding its transport
      // acknowledgement never applies the old intention to a later session.
      if (!response?.error) this.publish({ session: { ...this.state.session, paused: command.payload.paused } });
      if (response?.holdKey) await new Promise<void>(resolve => this.capturePausePending.set(response.holdKey!, resolve));
      if (response?.error) throw new Error(response.error);
    }
    else if (command.kind === 'set_translation_enabled' && this.state.session) this.publish({ session: { ...this.state.session, translationEnabled: command.payload.enabled } });
    else if (command.kind === 'save_credential_pool') { const input = command.payload.pool; const poolId = input.poolId ?? crypto.randomUUID(); const revision = input.expectedRevision + 1; const pool = { poolId, revision, policy: input.policy, metadata: input.metadata, members: input.members.map((member, position) => ({ ...member, memberId: member.memberId ?? crypto.randomUUID(), position })), contentHash: 'b'.repeat(64) }; resultId = `${poolId}@${revision}`; this.publish({ credentialPools: [...this.state.credentialPools, pool] }); }
    else if (command.kind === 'select_source' && this.state.session) this.publish({ session: { ...this.state.session, sources: this.state.session.sources.map(source => ({ ...source, selected: source.sourceId === command.payload.sourceId })) } });
    else if (command.kind === 'set_connection_suspended' || command.kind === 'set_purpose_runtime_enabled') { const response = this.runtimeResponses.shift(); if (!response) throw new Error('UI transport has no prepared runtime projection'); this.publish({ configuration: { ...this.state.configuration, ...response } }); }
    else if (command.kind === 'cancel_task') {
      const index = this.readingProjectionCancellations.findIndex(value => value.taskId === command.payload.taskId);
      if (index < 0) this.publish({ tasks: this.state.tasks.map(task => task.taskId === command.payload.taskId ? { ...task, status: 'cancelled', partialText: null } : task) });
      else {
        const response = this.readingProjectionCancellations.splice(index, 1)[0];
        this.readingProjectionFixtures[JSON.stringify([response.projection.unit.unitId, response.projection.unit.textRevision])] = structuredClone(response.projection);
        this.readingUnitFixtures[JSON.stringify([response.projection.unit.unitId, response.projection.unit.textRevision])] = structuredClone(response.projection.unit);
        this.publish(response.snapshot);
        return structuredClone(response.receipt);
      }
    }
    else throw new Error(`Unsupported controlled UI command: ${command.kind}`);
    this.state.commitRevision++;
    return { requestId: envelope.operationId, revision: this.state.commitRevision, replayed: false, resultId, ...(gameResults ? { gameResults } : {}) };
  }
  async openWindow(role: WindowRole) { this.windows.push(role); }
  async closeWindow() {}
  async targets() {
    this.targetRequests += 1;
    const response = this.targetResponses.shift();
    const value = structuredClone(response?.value ?? { candidates: this.targetCandidates, diagnostics: [] });
    if (response?.holdKey) await new Promise<void>(resolve => {
      const pending = this.targetPending.get(response.holdKey!) ?? new Set<() => void>(); pending.add(resolve); this.targetPending.set(response.holdKey!, pending);
    });
    if (response?.error) throw new Error(response.error);
    return value;
  }
  async launchGame(gameId: string, rowRevision: number) {
    this.launches.push({ gameId, rowRevision });
    const response = this.launchResponses.shift();
    if (response?.error) throw new Error(response.error);
  }
  async pickWindow(_gameId: string | null) { return this.targets(); }
  associate(request: AssociateRequest): Promise<Contracts.CommandReceipt>;
  associate(request: Contracts.ReviewedAssociateRequest): Promise<Contracts.AssociationOperationReceipt>;
  async associate(request: AssociateRequest | Contracts.ReviewedAssociateRequest): Promise<Contracts.CommandReceipt | Contracts.AssociationOperationReceipt> {
    if ('reviewId' in request) { if (!this.bindingFixtures.associate) throw new Error('受控后端尚未安装 reviewed 关联。'); return structuredClone(await this.bindingFixtures.associate(structuredClone(request))); }
    this.associations.push(structuredClone(request)); if (this.holdAssociation) await new Promise<void>(resolve => { this.associationResume = resolve; }); const response = this.associationResponses.shift(); if (response?.error) throw new Error(response.error); this.publish({ session: { sessionId: 'ui-fixture-session', gameId: request.gameId, sourceKind: 'hook', state: 'opening', paused: false, translationEnabled: true, sources: [] } }); return { requestId: request.operationId, revision: this.state.commitRevision, replayed: false, resultId: 'ui-fixture-session' }; }
  async pickExecutables() { return structuredClone(this.executableSelections); }
  async inspectExecutables(paths: string[]) { return paths.map(path => ({ path, displayName: path.split('/').at(-1) ?? path, architecture: null, environmentKey: null, bottleLocator: null })); }
  async onExecutableDrop(listener: (paths: string[]) => void) { this.dropListeners.add(listener); return () => { this.dropListeners.delete(listener); }; }
  async history(_gameId: string | null, cursor: string | null, _limit: number) { return structuredClone(cursor ? this.historyPages[cursor] ?? this.state.history : this.state.history); }
  // Actual Rust-produced pages may be served verbatim by their complete query
  // tuple, including the opaque cursor. Existing generic fixtures stay intact.
  historyQueryFixtures: Record<string, HistoryPage> = {};
  async historyPage(query: HistoryQuery): Promise<HistoryPage> {
    this.historyQueries.push(structuredClone(query));
    const prepared = this.historyQueryFixtures[JSON.stringify([query.filter.gameId, query.filter.sourceKind, query.filter.sessionId, query.cursor, query.limit])];
    if (prepared) { const result = structuredClone(prepared); await this.holdHistory('units', query.filter.gameId); return result; }
    const filter = query.filter; const scope = JSON.stringify(['units', filter.gameId, filter.sourceKind, filter.sessionId]);
    const matches = (unit: ReadingUnit) => (filter.gameId === null || unit.gameId === filter.gameId) && (filter.sourceKind === null || this.historySourceKind(unit) === filter.sourceKind) && (filter.sessionId === null || unit.sessionId === filter.sessionId);
    if (query.cursor && this.historyPages[query.cursor]) {
      if (this.legacyHistoryCursorScopes.get(query.cursor) !== scope) throw new Error('UI transport: history cursor scope mismatch');
      if (!Number.isInteger(query.limit) || query.limit < 1 || query.limit > 200) throw new Error('UI transport: invalid history page limit');
      const page = this.historyPages[query.cursor]; const units = page.units.filter(matches);
      if (units.length > query.limit) throw new Error('UI transport: prepared legacy page exceeds requested limit');
      if (page.nextCursor) this.legacyHistoryCursorScopes.set(page.nextCursor, scope);
      const result = structuredClone({ units, nextCursor: page.nextCursor }); await this.holdHistory('units', filter.gameId); return result;
    }
    const offset = this.pageOffset(query.cursor, scope, query.limit);
    const units = this.state.history.units.filter(matches); const selected = units.slice(offset, offset + query.limit);
    let nextCursor = offset + selected.length < units.length ? JSON.stringify({ scope, offset: offset + selected.length }) : this.state.history.nextCursor;
    if (nextCursor && this.historyPages[nextCursor]) this.legacyHistoryCursorScopes.set(nextCursor, scope);
    if (selected.length === 0 && offset > units.length) nextCursor = null;
    const result = structuredClone({ units: selected, nextCursor }); await this.holdHistory('units', filter.gameId); return result;
  }
  async historySessions(query: HistorySessionsQuery): Promise<HistorySessionsPage> {
    this.historySessionQueries.push(structuredClone(query));
    const scope = JSON.stringify(['sessions', query.filter.gameId, query.filter.sourceKind]);
    const offset = this.pageOffset(query.cursor, scope, query.limit);
    const sessions = new Map<string, HistorySessionSummary>();
    for (const unit of [...this.state.history.units, ...Object.values(this.historyPages).flatMap(page => page.units)]) {
      const current = sessions.get(unit.sessionId); const live = this.state.session?.sessionId === unit.sessionId ? this.state.session : null;
      if (!current) sessions.set(unit.sessionId, unit.sourceSummary ? structuredClone(unit.sourceSummary) : { sessionId: unit.sessionId, gameId: unit.gameId, sourceKind: this.historySourceKind(unit), state: live?.state ?? 'stopped', startedAtMs: unit.createdAtMs, endedAtMs: live?.state === 'active' || live?.state === 'opening' ? null : unit.createdAtMs });
      else { current.startedAtMs = Math.min(current.startedAtMs, unit.createdAtMs); if (current.endedAtMs !== null) current.endedAtMs = Math.max(current.endedAtMs, unit.createdAtMs); }
    }
    const filtered = [...sessions.values()].filter(session => (query.filter.gameId === null || session.gameId === query.filter.gameId) && (query.filter.sourceKind === null || session.sourceKind === query.filter.sourceKind)).sort((a, b) => b.startedAtMs - a.startedAtMs || a.sessionId.localeCompare(b.sessionId));
    const selected = filtered.slice(offset, offset + query.limit);
    const result = structuredClone({ sessions: selected, nextCursor: offset + selected.length < filtered.length ? JSON.stringify({ scope, offset: offset + selected.length }) : null }); await this.holdHistory('sessions', query.filter.gameId); return result;
  }

  readingUnitQueries: { unitId: string; textRevision: number }[] = [];
  readingUnitFixtures: Record<string, ReadingUnit> = {};
  async readingUnit(unitId: string, textRevision: number) { this.readingUnitQueries.push({ unitId, textRevision }); const unit = this.readingUnitFixtures[JSON.stringify([unitId, textRevision])] ?? [...this.state.history.units, ...Object.values(this.historyPages).flatMap(page => page.units)].find(unit => unit.unitId === unitId && unit.textRevision === textRevision); if (!unit) throw new Error('UI test unit not found'); return structuredClone(unit); }
  // Explicit query fixtures only. This transport never computes heads or engine
  // membership from the global task/history arrays or pretends to run SQLite.
  readingProjectionFixtures: Record<string, Contracts.ReadingUnitProjection> = {};
  readingProjectionPlans: { unitId?: string; textRevision?: number; value?: Contracts.ReadingUnitProjection; error?: string; holdKey?: string }[] = [];
  readingProjectionQueries: { unitId: string; textRevision: number; atMs: number; commitRevision: number }[] = [];
  readingProjectionPending = new Map<string, () => void>();
  readingProjectionActive = new Map<string, number>();
  readingProjectionMaxActive = new Map<string, number>();
  releaseReadingProjection(key: string) { const resolve = this.readingProjectionPending.get(key); if (!resolve) throw new Error('Controlled reading projection hold missing'); this.readingProjectionPending.delete(key); resolve(); }
  async readingUnitProjection(unitId: string, textRevision: number): Promise<Contracts.ReadingUnitProjection> {
    this.readingProjectionQueries.push({ unitId, textRevision, atMs: performance.now(), commitRevision: this.state.commitRevision });
    const owner = JSON.stringify([this.state.profileId, this.state.serverInstanceId, this.state.profileGeneration, unitId, textRevision]);
    const active = (this.readingProjectionActive.get(owner) ?? 0) + 1;
    this.readingProjectionActive.set(owner, active); this.readingProjectionMaxActive.set(owner, Math.max(this.readingProjectionMaxActive.get(owner) ?? 0, active));
    try {
      const index = this.readingProjectionPlans.findIndex(plan => (!plan.unitId || plan.unitId === unitId) && (plan.textRevision === undefined || plan.textRevision === textRevision));
      const plan = index < 0 ? undefined : this.readingProjectionPlans.splice(index, 1)[0];
      const explicit = plan?.value ?? this.readingProjectionFixtures[JSON.stringify([unitId, textRevision])];
      const unit = explicit?.unit ?? [this.state.current, ...this.state.history.units, ...Object.values(this.historyPages).flatMap(page => page.units)].find((unit): unit is ReadingUnit => unit !== null && unit.unitId === unitId && unit.textRevision === textRevision);
      if (!unit) throw new Error('UI transport: reading projection unit not prepared');
      const value = structuredClone(explicit ?? { profileId: this.state.profileId, serverInstanceId: this.state.serverInstanceId, profileGeneration: this.state.profileGeneration, commitRevision: this.state.commitRevision, unit, translationState: { unitId, textRevision, sourceHash: unit.sourceHash, engines: [] } });
      if (plan?.holdKey) await new Promise<void>(resolve => this.readingProjectionPending.set(plan.holdKey!, resolve));
      if (plan?.error) throw new Error(plan.error);
      return value;
    } finally { this.readingProjectionActive.set(owner, (this.readingProjectionActive.get(owner) ?? 1) - 1); }
  }
  retranslations: { unitId: string; textRevision: number; operationId: string }[] = [];
  readingProjectionCancellations: { taskId: string; snapshot: AppSnapshot; projection: Contracts.ReadingUnitProjection; receipt: Contracts.CommandReceipt }[] = [];
  readingProjectionRetranslations: { unitId: string; textRevision: number; snapshot?: AppSnapshot; projection?: Contracts.ReadingUnitProjection; receipts?: Contracts.TaskView[]; error?: string }[] = [];
  async retranslate(unitId: string, textRevision: number, operationId: string) {
    this.retranslations.push({ unitId, textRevision, operationId });
    const index = this.readingProjectionRetranslations.findIndex(value => value.unitId === unitId && value.textRevision === textRevision);
    if (index < 0) return [];
    const response = this.readingProjectionRetranslations.splice(index, 1)[0];
    if (response.error) throw new Error(response.error);
    if (!response.projection || !response.snapshot) throw new Error('UI transport: retranslation response not prepared');
    const key = JSON.stringify([unitId, textRevision]);
    this.readingProjectionFixtures[key] = structuredClone(response.projection);
    this.readingUnitFixtures[key] = structuredClone(response.projection.unit);
    this.publish(response.snapshot);
    return structuredClone(response.receipts ?? []);
  }
  async openWordCard(request: OpenWordCardRequest) {
    this.wordCardRequests.push(structuredClone(request));
    const unit = await this.readingUnit(request.unitId, request.textRevision);
    if (!unit.tokens[request.tokenIndex]) throw new Error('UI transport: stale word token');
    if (this.wordCard) this.wordCardClosedListeners.forEach(listener => listener(this.wordCard!.cardId));
    const cardId = `ui-word-card-${++this.wordCardSequence}`;
    this.wordCard = { cardId, unit, tokenIndex: request.tokenIndex, preferences: structuredClone(this.state.preferences) };
    this.wordCardRect = structuredClone(request.anchorRect); this.wordCardListeners.forEach(listener => listener());
    return cardId;
  }
  async wordCardSnapshot(): Promise<FrozenWordCard> { if (!this.wordCard) throw new Error('UI transport: word card closed'); return structuredClone(this.wordCard); }
  async wordCardLookup(cardId: string) { const card = await this.wordCardSnapshot(); if (card.cardId !== cardId) throw new Error('UI transport: stale word card'); const lookup = await this.lookup(card.unit.unitId, card.unit.textRevision, card.tokenIndex); if (this.wordCard?.cardId !== cardId) throw new Error('UI transport: stale lookup'); return lookup; }
  async wordCardAction(cardId: string, intent: WordCardIntent) { const card = await this.wordCardSnapshot(); if (card.cardId !== cardId) throw new Error('UI transport: stale word card'); await this.freezeLearning(card.unit.unitId, card.unit.textRevision, card.tokenIndex); if (intent === 'context_word') await this.study(card.unit.unitId, card.unit.textRevision, 'context_word', card.tokenIndex); await this.closeWordCard(cardId); }
  async closeWordCard(cardId: string) { if (this.wordCard?.cardId !== cardId) return; this.wordCard = null; this.wordCardListeners.forEach(listener => listener()); this.wordCardClosedListeners.forEach(listener => listener(cardId)); }
  async onWordCardClosed(listener: (cardId: string) => void) { this.wordCardClosedListeners.add(listener); return () => { this.wordCardClosedListeners.delete(listener); }; }
  async lookup(unitId: string, textRevision: number, tokenIndex: number) { const unit = this.state.history.units.find(item => item.unitId === unitId) ?? this.state.learningObject?.unit; if (!unit) throw new Error('UI fixture not found'); return { unitId, textRevision, tokenIndex, token: unit.tokens[tokenIndex], entries: structuredClone(this.dictionaryEntries), resourceVersion: 'ui-controlled-fixture-no-real-dictionary' }; }
  async testConfiguration(request: ConfigurationTestRequest) {
    this.configurationTests.push(structuredClone(request));
    const response = this.configurationTestResponses.shift();
    if (response?.holdKey) { const key = response.holdKey; await new Promise<void>(resolve => { this.configurationTestPending.set(key, resolve); }); }
    // No subscription delivery: force the UI to recover via its explicit refresh.
    if (response?.patch) this.state = { ...this.state, ...structuredClone(response.patch), revision: this.state.revision + 1 };
    if (response?.error) throw new Error(response.error);
    return { ...request, status: 'not_run', message: 'UI transport does not call a real service', modelIds: [] };
  }
  async storeCredential(_input: CredentialInput): Promise<CredentialReference> { throw new Error('UI transport does not access Keychain'); }
  async credentialRevocationImpact(credentialId: string): Promise<CredentialRevocationImpact> { this.credentialImpactRequests.push(credentialId); if (!this.credentialImpact || this.credentialImpact.credentialId !== credentialId) throw new Error('UI transport has no prepared credential impact'); return structuredClone(this.credentialImpact); }
  async revokeCredential(request: CredentialRevocationRequest) { this.revocations.push(structuredClone(request)); const response = this.revocationResponses.shift(); if (!response) throw new Error('UI transport has no prepared revocation response'); this.publish(response.patch); if (response.error) throw new Error(response.error); return { requestId: request.operationId, revision: this.state.commitRevision, resultId: request.credentialId, replayed: this.revocations.length > 1 }; }
  async resources() { return []; }
  async freezeLearning(unitId: string, textRevision: number, tokenIndex: number | null) { const unit = await this.readingUnit(unitId, textRevision); const object = { unit: structuredClone(unit), tokenIndex }; this.publish({ learningObject: object }); this.windows.push('learning'); return object; }
  async study(unitId: string, textRevision: number, purpose: string, tokenIndex: number | null) { this.studies.push({ unitId, textRevision, purpose, tokenIndex }); return { taskId: 'ui-no-model-request', unitId, textRevision, purpose, status: 'not_run', errorCode: 'UI_TEST_ONLY', partialText: null, providerLabel: 'UI-only transport', attemptId: null }; }
}
