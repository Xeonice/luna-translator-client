import type { Configuration } from '../../bridge/contracts.generated';

export const scheduleLimits = {
  concurrency: { min: 1, max: 8, fallback: 1, label: '用途总并发' },
  queueCapacity: { min: 1, max: 128, fallback: 32, label: '待执行队列容量' },
  retries: { min: 0, max: 3, fallback: 0, label: '最大重试次数' },
} as const;

// Draft feedback only. Rust still validates and resolves the saved configuration.
export function configurationDraftIssues(configuration: Configuration): string[] {
  const issues: string[] = [];
  for (const binding of configuration.bindings) {
    for (const [key, limit] of Object.entries(scheduleLimits)) {
      const value = binding.schedule[key];
      if (value === undefined) continue;
      if (typeof value !== 'number' || !Number.isInteger(value) || value < limit.min || value > limit.max) issues.push(`${limit.label}须为 ${limit.min}–${limit.max} 的整数。`);
    }
  }
  for (const connection of configuration.connections) {
    const proxy = connection.networkMetadata.proxyUrl;
    if (typeof proxy !== 'string' || !proxy.trim()) continue;
    try { const url = new URL(proxy); if (url.username || url.password) issues.push('代理地址不能包含明文凭据。'); }
    catch { issues.push('代理地址需要包含完整协议与主机。'); }
  }
  return [...new Set(issues)];
}
