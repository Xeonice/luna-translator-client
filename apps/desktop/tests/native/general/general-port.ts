/** Controlled visual input, imported only by the independent verification entry.
 * No actual SQLite, login item, HTTP, capture, hook or production Tauri transport. */
import { DesignPort, emptySnapshot } from '../../support/design-port';
import type { Game, LoginStartupState, ScopedPreferences } from '../../../src/bridge/contracts.generated';
import type { GeneralVisualContext } from './native-context';
import fixture from './fixture-data.json';

export class GeneralVisualPort extends DesignPort {
  readonly forbiddenEffects: string[] = [];
  readonly visualSaveCalls: Parameters<DesignPort['dispatch']>[0][] = [];

  constructor(readonly visualContext: GeneralVisualContext) {
    super('reading');
    this.state = emptySnapshot();
    this.state.profileId = 'general-visual-memory';
    this.state.serverInstanceId = 'general-visual-memory-instance';
    this.state.preferences = { ...this.state.preferences, ...fixture.preferences, theme: visualContext.theme, scopeKey: 'global', overrides: {} };
    const displayName = visualContext.stateId.endsWith('-long-text') ? fixture.longGameName : fixture.gameName;
    const game: Game = { gameId: fixture.gameId, displayName, originalTitle: null, sourceLanguage: 'ja', coverAssetRef: null, rowRevision: 1,
      target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } };
    this.state.games = visualContext.stateId.endsWith('-empty') ? [] : [game];
    this.loginStartupFixture = { status: 'not_found', canChange: false, errorCode: 'service_not_found', detail: fixture.startup.detail };
    this.serviceFixtures.saved = async query => ({ profileGeneration: this.state.profileGeneration,
      appliedHeadRevision: this.state.configuration.appliedHeadRevision,
      selection: { requestedScope: query.scope, effectiveScope: { kind: 'global' }, selectionRevision: 0,
        effectiveSelectionRevision: 0, main: null, comparisons: [], purposeEnabled: false }, rows: [] });
    if (visualContext.stateId.endsWith('-loading') && visualContext.subcase === 'initial') {
      this.scopedPreferenceResponses.push({ scopeKey: 'global', holdKey: 'general-visual-initial-read' });
    }
    if (visualContext.stateId.endsWith('-restore-inheritance')) {
      const scopeKey = `game:${fixture.gameId}`;
      this.scopedPreferenceSaves.set(scopeKey, { saved: { ...structuredClone(this.state.preferences), scopeKey,
        showRuby: false, overrides: { showRuby: true } }, headRevision: 1, preferenceRevisionId: 'general-visual-game-override-1' });
    }
  }

  override async preferencesScope(scopeKey: string): Promise<ScopedPreferences> {
    const view = await super.preferencesScope(scopeKey);
    // The test transport must project the actual global inheritance draft.
    // Existing DesignPort's generic fixture starts from the local saved value.
    if (view.inheritedDraft) view.inheritedDraft = { ...structuredClone(view.global), scopeKey, overrides: {} };
    return view;
  }

  override async dispatch(input: Parameters<DesignPort['dispatch']>[0]): ReturnType<DesignPort['dispatch']> {
    if (input.command.kind !== 'save_scoped_preferences') {
      this.forbiddenEffects.push(input.command.kind);
      throw new Error('This controlled General visual transport does not install that operation.');
    }
    this.visualSaveCalls.push(structuredClone(input));
    return super.dispatch(input);
  }

  override async setLoginStartup(_enabled: boolean): Promise<LoginStartupState> {
    this.forbiddenEffects.push('set_login_startup');
    throw new Error('The visual renderer cannot change an operating-system login item.');
  }
}
