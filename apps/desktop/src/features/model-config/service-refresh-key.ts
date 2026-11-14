import type { AppSnapshot } from '../../bridge/contracts.generated';

/** Service readiness changes independently of incoming text and partial output. */
export function serviceSelectionRefreshKey(snapshot: AppSnapshot): string {
  return JSON.stringify([
    snapshot.profileGeneration,
    snapshot.configuration.draftHeadRevision,
    snapshot.configuration.appliedHeadRevision,
    snapshot.configuration.runtimeConnections.map(row => [row.connectionId, row.controlRevision, row.suspended]),
    snapshot.configuration.runtimePurposes.map(row => [row.scopeKey, row.purpose, row.authorizationEpoch, row.runtimeEnabled]),
    snapshot.configurationTests.map(row => [row.testId, row.status, row.stale, row.comparisonCoverage]),
    snapshot.credentials.map(row => [row.credentialId, row.lifecycleRevision, row.state]),
    snapshot.credentialPools.map(row => [row.poolId, row.revision]),
  ]);
}
