/** Runs against the real Tauri port with only the native invoke boundary replaced.
 * This proves request routing and lossless transport, not Rust or OS behavior.
 */
import { TauriApplicationPort } from '../../src/bridge/tauri-port';
import type * as Contracts from '../../src/bridge/contracts.generated';

export async function verifyBindingPortRoutes() {
  const surface = window as unknown as { __TAURI_INTERNALS__?: { invoke: (command: string, args: unknown) => Promise<unknown> } };
  const previous = surface.__TAURI_INTERNALS__;
  const calls: { command: string; args: unknown }[] = [];
  const sentinel = { boundaryReceipt: 'controlled-native-boundary' };
  surface.__TAURI_INTERNALS__ = { invoke: async (command, args) => { calls.push({ command, args }); return sentinel; } };
  const assert = (condition: boolean, message: string) => { if (!condition) throw new Error(message); };
  try {
    const port = new TauriApplicationPort();
    const query: Contracts.AssociationReviewQuery = { candidateId: 'candidate-actual-id', gameId: null, expectedProfileGeneration: 7, expectedActiveSessionId: null, attributionIntent: 'auto_match' };
    const authority: Contracts.ReadingWindowEffectAuthority = { profile: { profileId: 'profile', serverInstanceId: 'server', profileGeneration: 7 }, sessionId: 'session', setRevision: '9007199254740993', operationId: 'operation' };
    const request: Contracts.BeginReadingRequest = { operationId: 'operation', sessionId: 'session', expectedProfileGeneration: 7, expectedSetRevision: '9007199254740993', sourceIds: ['source-b', 'source-a'], remember: false };
    const checks: [string, unknown, () => Promise<unknown>][] = [
      ['application_review_association', { query }, () => port.reviewAssociation(query)],
      ['application_review_association', { query: { ...query, attributionIntent: 'session_only' } }, () => port.reviewAssociation({ ...query, attributionIntent: 'session_only' })],
      ['application_associate_reviewed', { request: { operationId: 'operation', reviewId: 'opaque-review' } }, () => port.associate({ operationId: 'operation', reviewId: 'opaque-review' })],
      ['application_association_progress', { query: { operationId: 'operation', expectedProfileGeneration: 7 } }, () => port.associationProgress({ operationId: 'operation', expectedProfileGeneration: 7 })],
      ['application_source_selection', { query: { sessionId: 'session', expectedProfileGeneration: 7 } }, () => port.sourceSelection({ sessionId: 'session', expectedProfileGeneration: 7 })],
      ['application_begin_reading', { request }, () => port.beginReading(request)],
      ['application_retry_reading_window_effects', { authority }, () => port.retryReadingWindowEffects(authority)],
      ['application_associate', { request: { operationId: 'legacy-operation', gameId: null, candidateId: 'candidate', confirmSwitch: false } }, () => port.associate({ operationId: 'legacy-operation', gameId: null, candidateId: 'candidate', confirmSwitch: false })],
    ];
    for (const [command, args, invoke] of checks) {
      const count = calls.length, result = await invoke();
      assert(calls.length === count + 1, command + ' must invoke exactly once');
      assert(calls.at(-1)?.command === command, command + ' route differs');
      assert(JSON.stringify(calls.at(-1)?.args) === JSON.stringify(args), command + ' changed the exact request');
      assert(result === sentinel, command + ' replaced the native receipt');
    }
    const submitCount = calls.filter(call => call.command === 'application_begin_reading').length;
    assert(submitCount === 1, 'Window-effect retry must not resubmit the source set');
    assert(request.expectedSetRevision === '9007199254740993', 'Set revision lost integer precision');
    assert(request.sourceIds.join(',') === 'source-b,source-a', 'Source order changed in transport');
    return { status: 'passed', assertions: checks.length * 4 + 3, calls };
  } finally { if (previous) surface.__TAURI_INTERNALS__ = previous; else delete surface.__TAURI_INTERNALS__; }
}
