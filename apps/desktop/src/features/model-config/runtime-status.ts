import type { PurposeRuntimeView } from '../../bridge/contracts.generated';

/** Render only the Rust-projected authorization state, never infer it from a draft. */
export function purposeRuntimeStatus(view: PurposeRuntimeView | null | undefined): string {
  if (!view) return '运行许可未建立';
  if (!view.enabledIntent) return '已应用意图未启用';
  if (!view.runtimeEnabled) return '已即时停用';
  if (!view.authorizedEngineIds.length) return '没有获准运行的引擎';
  return view.blockedConnectionIds.length ? '部分引擎获准运行' : '本地运行许可开放';
}
