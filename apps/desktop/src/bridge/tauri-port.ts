import { listen } from '@tauri-apps/api/event';
import { Channel, invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { AppSnapshot, AssociateRequest, CommandEnvelope, CommandReceipt, ConfigurationTestRequest, ConfigurationTestResult, CredentialInput, CredentialReference, ExecutableSelection, HistoryPage, ProjectionUpdate, ReadingUnit, ResourceInfo, TargetCandidates, TaskView, LearningObject, WordLookup } from './contracts.generated';
import type { ApplicationPort, WindowRole } from './application-port';
import { PreviewSubscriptionClient } from './preview-subscriptions';
import type { PreviewUpdate, PreviewProjection, PreviewStartRequest, PreviewControlRequest } from './contracts.generated';

export class TauriApplicationPort implements ApplicationPort {
  serviceCatalog() { return invoke<import('./contracts.generated').ServiceCatalog>('application_service_catalog'); }
  serviceForm(request: import('./contracts.generated').ServiceFormRequest) { return invoke<import('./contracts.generated').ServiceFormProjection>('application_service_form', { request }); }
  serviceDraft(query: import('./contracts.generated').ServiceDraftQuery) { return invoke<import('./contracts.generated').ServiceDraftEditorProjection>('application_service_draft', { query }); }
  testSavedService(input: import('./contracts.generated').SavedServiceTestRequest, operationId?: string, selectedTokenIndex?: number) { return invoke<import('./contracts.generated').ServiceTestResult>('application_test_saved_service', { input, operationId: operationId ?? null, selectedTokenIndex: selectedTokenIndex ?? null }); }
  cancelSavedServiceTest(operationId: string, expectedProfileGeneration: number) { return invoke<void>('application_cancel_saved_service_test', { operationId, expectedProfileGeneration }); }
  serviceConfiguration() { return invoke<import('./contracts.generated').ServiceConfigurationSnapshot>('application_service_configuration'); }
  saveServiceDraft(input: import('./contracts.generated').SaveServiceDraftInput) { return invoke<import('./contracts.generated').ServiceDraftReceipt>('application_save_service_draft', { input }); }
  savedServices(query: import('./contracts.generated').SavedServiceQuery) { return invoke<import('./contracts.generated').SavedServiceProjection>('application_saved_services', { query }); }
  selectSavedServices(input: import('./contracts.generated').SelectSavedServicesInput) { return invoke<import('./contracts.generated').ServiceSelectionReceipt>('application_select_saved_services', { input }); }
  setServicePurposeEnabled(input: import('./contracts.generated').SetServicePurposeEnabledInput) { return invoke<import('./contracts.generated').ServiceSelectionReceipt>('application_set_service_purpose_enabled', { input }); }
  restoreServiceInheritance(input: import('./contracts.generated').RestoreServiceInheritanceInput) { return invoke<import('./contracts.generated').ServiceSelectionReceipt>('application_restore_service_inheritance', { input }); }
  storeServiceCredential(input: import('./contracts.generated').ServiceCredentialRequest) { return invoke<CredentialReference>('application_store_service_credential', { input }); }
  inspectGoogleAuthorization(context: import('./contracts.generated').GoogleInteractiveContext) { return invoke<import('./contracts.generated').GoogleAuthorizationProjection>('application_google_authorization_inspect', { context }); }
  startGoogleLogin(input: import('./contracts.generated').StartGoogleLoginInput) { return invoke<import('./contracts.generated').GoogleLoginOperationProjection>('application_google_authorization_start', { input }); }
  googleLoginStatus(query: import('./contracts.generated').GoogleLoginOperationQuery) { return invoke<import('./contracts.generated').GoogleLoginOperationProjection>('application_google_authorization_status', { query }); }
  cancelGoogleLogin(query: import('./contracts.generated').GoogleLoginOperationQuery) { return invoke<import('./contracts.generated').GoogleLoginOperationProjection>('application_google_authorization_cancel', { query }); }
  revokeGoogleAuthorization(input: import('./contracts.generated').RevokeGoogleAuthorizationInput) { return invoke<import('./contracts.generated').GoogleRevokeProjection>('application_google_authorization_revoke', { input }); }
  private readonly previewSubscriptions = new PreviewSubscriptionClient((leaseId, generation, afterSequence) => invoke<PreviewUpdate>('application_preview_latest', { leaseId, generation, afterSequence }));
  private targetDiscovery: Promise<TargetCandidates> | undefined;
  snapshot() { return invoke<AppSnapshot>('application_snapshot'); }
  dispatch(envelope: CommandEnvelope) { return invoke<CommandReceipt>('application_dispatch', { envelope }); }
  async subscribe(listener: (update: ProjectionUpdate) => void, onError: (error: unknown) => void) {
    let active = true;
    const channel = new Channel<ProjectionUpdate>();
    channel.onmessage = update => { if (active) listener(update); };
    const subscriptionId = await invoke<string>('application_subscribe', { channel }).catch(error => { active = false; onError(error); throw error; });
    return () => { active = false; void invoke('application_unsubscribe', { subscriptionId }).catch(onError); };
  }
  windowCapabilities() { return invoke<import('./contracts.generated').WindowCapabilities>('application_window_capabilities'); }
  loginStartup() { return invoke<import('./contracts.generated').LoginStartupState>('application_login_startup'); }
  setLoginStartup(enabled: boolean) { return invoke<import('./contracts.generated').LoginStartupState>('application_set_login_startup', { enabled }); }
  preferencesScope(scopeKey: string) { return invoke<import('./contracts.generated').ScopedPreferences>('application_preferences_scope', { scopeKey }); }
  gameWindowBinding() { return invoke<import('./contracts.generated').GameWindowBindingProjection>('application_game_window_binding'); }
  showAssociatedGame(request: import('./contracts.generated').GameWindowActivationRequest) { return invoke<import('./contracts.generated').GameWindowActivationReceipt>('application_show_associated_game', { request }); }
  beginHudDrag() { return invoke<void>('application_begin_hud_drag'); }
  fitHudContent(height: number) { return invoke<import('./contracts.generated').GeometryProjection>('application_fit_hud_content', { height }); }
  resetHudPosition() { return invoke<import('./contracts.generated').GeometryProjection>('application_reset_hud_position'); }
  setShortcutRecording(recording: boolean) { return invoke<import('./contracts.generated').ShortcutProjection>('application_shortcut_recording', { recording }); }
  readGameCover(request: import('./contracts.generated').GameCoverRequest) { return invoke<import('./contracts.generated').GameCoverResult>('application_game_cover', { request }); }
  previewSession() { return invoke<PreviewProjection>('application_preview_session'); }
  attachWindowPreview(projection: PreviewProjection, listener: (update: PreviewUpdate) => void): () => void {
    return this.previewSubscriptions.attach(projection, listener);
  }
  previewCapability() { return invoke<import('./contracts.generated').PreviewCapability>('application_preview_capability'); }
  startWindowPreview(request: PreviewStartRequest) { return invoke<PreviewProjection>('application_preview_start', { request }); }
  selectWindowPreview(request: PreviewStartRequest) { return invoke<PreviewProjection>('application_preview_select_window', { request }); }
  controlWindowPreview(request: PreviewControlRequest) { return invoke<PreviewProjection>('application_preview_control', { request }); }
  sessionRecords(query: import('./contracts.generated').SessionRecordQuery) { return invoke<import('./contracts.generated').SessionRecordPage>('application_session_records', { query }); }
  prepareSessionExport(request: import('./contracts.generated').SessionExportRequest) { return invoke<import('./contracts.generated').SessionExportPlan>('application_session_export_prepare', { request }); }
  confirmSessionExport(exportId: string) { return invoke<import('./contracts.generated').SessionExportReceipt>('application_session_export_confirm', { exportId }); }
  cancelSessionExport(exportId: string) { return invoke<void>('application_session_export_cancel', { exportId }); }
  cycleReadingDisplayMode() { return invoke<void>('application_cycle_reading_display_mode'); }
  copyReadingUnit(unitId: string, textRevision: number) { return invoke<void>('application_copy_reading_unit', { unitId, textRevision }); }
  retryLexical(unitId: string, textRevision: number) { return invoke<void>('application_retry_lexical', { unitId, textRevision }); }
  openWindow(role: WindowRole) { return invoke<void>('application_window', { role }); }
  openMainPage(intent: import('./contracts.generated').MainPageIntent) { return invoke<void>('application_open_main_page', { intent }); }
  async onMainPageRequested(listener: (request: import('./contracts.generated').MainPageRequest) => void) {
    const unsubscribe = await listen<import('./contracts.generated').MainPageRequest>('application-main-page', event => listener(event.payload));
    try {
      const pending = await invoke<import('./contracts.generated').MainPageRequest | null>('application_main_page');
      if (pending) listener(pending);
      return unsubscribe;
    } catch (error) { unsubscribe(); throw error; }
  }
  openSettingsPage(intent: import('./contracts.generated').SettingsPageIntent) { return invoke<void>('application_open_settings_page', { intent }); }
  async onSettingsPageRequested(listener: (request: import('./contracts.generated').SettingsPageRequest) => void) {
    const unsubscribe = await listen<import('./contracts.generated').SettingsPageRequest>('application-settings-page', event => listener(event.payload));
    try {
      const pending = await invoke<import('./contracts.generated').SettingsPageRequest | null>('application_settings_page');
      if (pending) listener(pending);
      return unsubscribe;
    } catch (error) { unsubscribe(); throw error; }
  }
  closeWindow() { return getCurrentWindow().close(); }
  targets() {
    if (!this.targetDiscovery) {
      const pending = invoke<TargetCandidates>('application_targets');
      this.targetDiscovery = pending;
      // Reuse only the in-flight transport, including StrictMode reattachment.
      void pending.then(() => { if (this.targetDiscovery === pending) this.targetDiscovery = undefined; }, () => { if (this.targetDiscovery === pending) this.targetDiscovery = undefined; });
    }
    return this.targetDiscovery;
  }
  pickWindow(gameId: string | null) { return invoke<TargetCandidates>('application_pick_window', { gameId }); }
  associate(request: AssociateRequest): Promise<CommandReceipt>;
  associate(request: import('./contracts.generated').ReviewedAssociateRequest): Promise<import('./contracts.generated').AssociationOperationReceipt>;
  associate(request: AssociateRequest | import('./contracts.generated').ReviewedAssociateRequest) {
    return 'reviewId' in request
      ? invoke<import('./contracts.generated').AssociationOperationReceipt>('application_associate_reviewed', { request })
      : invoke<CommandReceipt>('application_associate', { request });
  }
  reviewAssociation(query: import('./contracts.generated').AssociationReviewQuery) { return invoke<import('./contracts.generated').AssociationReview>('application_review_association', { query }); }
  associationProgress(query: import('./contracts.generated').AssociationProgressQuery) { return invoke<import('./contracts.generated').AssociationProgress>('application_association_progress', { query }); }
  sourceSelection(query: import('./contracts.generated').SourceSelectionQuery) { return invoke<import('./contracts.generated').SourceSelectionProjection>('application_source_selection', { query }); }
  beginReading(request: import('./contracts.generated').BeginReadingRequest) { return invoke<import('./contracts.generated').BeginReadingResult>('application_begin_reading', { request }); }
  retryReadingWindowEffects(authority: import('./contracts.generated').ReadingWindowEffectAuthority) { return invoke<import('./contracts.generated').ReadingWindowEffectsResult>('application_retry_reading_window_effects', { authority }); }
  pickExecutables() { return invoke<ExecutableSelection[]>('application_pick_executables'); }
  inspectExecutables(paths: string[]) { return invoke<ExecutableSelection[]>('application_inspect_executables', { paths }); }
  onExecutableDrop(listener: (paths: string[]) => void) { return getCurrentWebview().onDragDropEvent(event => { if (event.payload.type === 'drop') listener(event.payload.paths); }); }
  history(gameId: string | null, cursor: string | null, limit: number) { return invoke<HistoryPage>('application_history', { gameId, cursor, limit }); }
  historyPage(query: import('./contracts.generated').HistoryQuery) { return invoke<HistoryPage>('application_history_page', { query }); }
  historySessions(query: import('./contracts.generated').HistorySessionsQuery) { return invoke<import('./contracts.generated').HistorySessionsPage>('application_history_sessions', { query }); }
  readingUnit(unitId: string, textRevision: number) { return invoke<ReadingUnit>('application_reading_unit', { unitId, textRevision }); }
  readingUnitProjection(unitId: string, textRevision: number) { return invoke<import('./contracts.generated').ReadingUnitProjection>('application_reading_unit_projection', { unitId, textRevision }); }
  retranslate(unitId: string, textRevision: number, operationId: string) { return invoke<TaskView[]>('application_retranslate', { unitId, textRevision, operationId }); }
  openWordCard(request: import('./contracts.generated').OpenWordCardRequest) { return invoke<string>('application_word_card_open', { request }); }
  wordCardSnapshot() { return invoke<import('./contracts.generated').FrozenWordCard>('application_word_card_snapshot'); }
  wordCardLookup(cardId: string) { return invoke<WordLookup>('application_word_card_lookup', { cardId }); }
  wordCardAction(cardId: string, intent: import('./contracts.generated').WordCardIntent) { return invoke<void>('application_word_card_action', { cardId, intent }); }
  closeWordCard(cardId: string) { return invoke<void>('application_word_card_close', { cardId }); }
  onWordCardClosed(listener: (cardId: string) => void) { return listen<string>('word-card-closed', event => listener(event.payload)); }
  lookup(unitId: string, textRevision: number, tokenIndex: number) { return invoke<WordLookup>('application_lookup', { unitId, textRevision, tokenIndex }); }
  testConfiguration(request: ConfigurationTestRequest) { return invoke<ConfigurationTestResult>('application_test_configuration', { request }); }
  storeCredential(input: CredentialInput) { return invoke<CredentialReference>('application_store_credential', { input }); }
  credentialRevocationImpact(credentialId: string) { return invoke<import('./contracts.generated').CredentialRevocationImpact>('application_credential_revocation_impact', { credentialId }); }
  revokeCredential(request: import('./contracts.generated').CredentialRevocationRequest) { return invoke<CommandReceipt>('application_revoke_credential', { request }); }
  resources() { return invoke<ResourceInfo[]>('application_resources'); }
  freezeLearning(unitId: string, textRevision: number, tokenIndex: number | null) { return invoke<LearningObject>('application_learning_object', { unitId, textRevision, tokenIndex }); }
  study(unitId: string, textRevision: number, purpose: string, tokenIndex: number | null) { return invoke<TaskView>('application_study', { unitId, textRevision, purpose, tokenIndex }); }
}
