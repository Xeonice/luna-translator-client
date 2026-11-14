import type { ReadingUnit, Session } from '../../bridge/contracts.generated';

export function readingOriginLabel(unit: ReadingUnit) {
  return unit.sourceSummary?.sourceKind === 'manual' ? '手动输入'
    : unit.sourceSummary?.sourceKind === 'hook' ? '游戏原文' : '来源未记录';
}

/** Labels format the authoritative session state; saved source choices do not imply a live connection. */
export function sessionPresentation(session: Session | null) {
  const live = session?.state === 'opening' || session?.state === 'active';
  let label = '等待连接游戏';
  if (session) {
    switch (session.state) {
      case 'opening': label = '正在接入游戏'; break;
      case 'active':
        label = session.paused ? '自动输入已暂停'
          : session.sources.some(source => source.selected) ? '正在接收正文' : '等待选择正文流';
        break;
      case 'stopped': label = '游戏取文已断开'; break;
      case 'failed': label = '游戏接入失败'; break;
      case 'crashed': label = '上次游戏取文已中断'; break;
      default: label = '游戏取文状态未知';
    }
  }
  return { label, live, canPause: session?.state === 'active', canSelectSources: session?.state === 'active' && session.sources.length > 0 };
}
