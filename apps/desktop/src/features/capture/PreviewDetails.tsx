import { useLayoutEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';
import type { PreviewCapability, PreviewConsumer, PreviewFrame, PreviewProjection, TargetCandidate } from '../../bridge/contracts.generated';
import { Button, Sheet } from '../../ui/primitives/Controls';
import styles from './styles.module.css';

export type PreviewTargetFacts = Partial<Pick<TargetCandidate, 'windowTitle' | 'executablePath' | 'windowsPid' | 'processCreatedU64' | 'environmentKey' | 'bottleLabel' | 'windowHandle' | 'macosHostPid' | 'macosWindowId'>> & { gameTitle?: string; verifiedAtMs?: string | null; origin?: 'candidate' | 'game_record' };
export type PreviewFrameFacts = Pick<PreviewFrame, 'capturedAtMs' | 'sequence' | 'width' | 'height' | 'encoding'>;
type FactGroup = { heading: string; rows: [string, string][] };
const missing = '未提供（当前接口未包含此事实）';
const privacy = '只预览已明确选择的游戏窗口，不采音频、不上传到模型。持续画面不录制、不连续保存；仅在正式关联、已有录屏权限且游戏没有本机封面时保存首张有效帧为静态封面，不覆盖已有封面。预览与 Hook 取文独立；查看、关闭或复制详情不改变当前采集。';
function fact(value: string | number | null | undefined) { return value === null || value === undefined || value === '' ? missing : String(value); }
function fullTime(value: string | null | undefined) {
  if (!value) return '尚无成功画面';
  const date = new Date(Number(value));
  return Number.isFinite(date.getTime()) ? `${date.toISOString()}（${value} ms）` : value;
}

/** A read-only view of the existing subscription. Frame bytes never enter it. */
export function PreviewDetails({ open, onClose, dialogId, anchorRef, title, detail, identityDetail, targetFacts, consumer, consumerId, stateLabel, guidance, projection, capability, frame, error, authorizationOpen }: {
  open: boolean; onClose: () => void; dialogId: string; anchorRef: RefObject<HTMLButtonElement | null>;
  title: string; detail: string; identityDetail?: string; targetFacts?: PreviewTargetFacts;
  consumer: PreviewConsumer; consumerId: string; stateLabel: string; guidance: string; projection: PreviewProjection | null;
  capability: PreviewCapability | null; frame: PreviewFrameFacts | null; error: string | null; authorizationOpen: boolean;
}) {
  const [anchor, setAnchor] = useState<{ right: number; top: number }>();
  const [copyStatus, setCopyStatus] = useState('');
  const [copying, setCopying] = useState(false);
  const copyPending = useRef(false);
  const copyDisplayEpoch = useRef(0);
  useLayoutEffect(() => {
    copyDisplayEpoch.current += 1;
    if (!open) return;
    setCopyStatus('');
    const position = () => {
      const bounds = anchorRef.current?.getBoundingClientRect();
      if (bounds) setAnchor({ right: bounds.right, top: bounds.top });
    };
    position();
    window.addEventListener('resize', position);
    return () => { copyDisplayEpoch.current += 1; window.removeEventListener('resize', position); };
  }, [open, anchorRef]);
  const groups: FactGroup[] = [
    { heading: '采集与状态', rows: [
      ['当前状态', stateLabel], ['状态说明', guidance], ['当前展示画面时间', fullTime(frame?.capturedAtMs)],
      ['回执中的采集时间', projection?.capturedAtMs ? fullTime(projection.capturedAtMs) : '尚无成功画面'],
      ['内容变化时间', projection?.contentChangedAtMs ? fullTime(projection.contentChangedAtMs) : missing],
      ['来源', frame ? '当前预览订阅收到的窗口画面；非实时状态仅保留最后成功画面。' : '当前没有可展示画面，状态回执不代表实时画面。'],
      ['会话归属', consumer === 'candidate' ? '候选核对 · 未确认接入' : '会话常驻 · 跨页面继续'],
      ['消费者身份', consumerId], ['预览租约', fact(projection?.leaseId)],
      ['代次 / 帧序号', `${fact(projection?.generation)} / ${fact(frame?.sequence)}`],
      ['画面尺寸 / 编码', frame ? `${frame.width} × ${frame.height} · ${frame.encoding}` : missing],
    ] },
    { heading: '窗口身份', rows: [
      ['完整游戏名称', fact(targetFacts?.gameTitle)], ['完整预览标题', title],
      ['原始窗口标题', fact(targetFacts?.windowTitle)], ['目标摘要', detail], ['完整身份说明', fact(identityDetail)],
      ['字段来源', targetFacts?.origin === 'candidate' ? '当前目标候选回执；窗口资料来自当前候选。' : targetFacts?.origin === 'game_record' ? '游戏名称、环境和路径来自可编辑的游戏资料；窗口标题与核对时间来自现有关联回执，资料路径不是本次 Native 核对结果。' : missing],
      [targetFacts?.origin === 'game_record' ? '资料环境标识 / Bottle' : '候选环境标识 / Bottle', `${fact(targetFacts?.environmentKey)} / ${fact(targetFacts?.bottleLabel)}`],
      [targetFacts?.origin === 'game_record' ? '程序文件名（游戏资料）' : '实际 EXE（候选回执）', targetFacts?.executablePath ? targetFacts.executablePath.split(/[\\/]/).at(-1) || targetFacts.executablePath : missing],
      [targetFacts?.origin === 'game_record' ? '完整路径（游戏资料）' : '完整路径（候选回执）', fact(targetFacts?.executablePath)], ['Windows PID', fact(targetFacts?.windowsPid)],
      ['Windows 进程创建标记', fact(targetFacts?.processCreatedU64)], ['Windows 窗口句柄', fact(targetFacts?.windowHandle)],
      ['宿主 PID', fact(targetFacts?.macosHostPid)], ['宿主进程启动时间', missing],
      ['macOS 窗口身份', fact(targetFacts?.macosWindowId)],
      ['关联核对时间', targetFacts?.verifiedAtMs ? fullTime(targetFacts.verifiedAtMs) : missing],
      ['目标身份摘要 / hash', fact(projection?.targetIdentityHash)],
      ['核对来源', '现有目标资料与预览回执；hash 仅匹配订阅对象，不作为额外的 Native 归属证明。'],
    ] },
    { heading: '诊断原文', rows: [
      ['预览状态码', fact(projection?.state)], ['预览回执权限', fact(projection?.permission)], ['能力检查权限', fact(capability?.permission)],
      ['平台支持', capability ? capability.supported ? '支持' : '不支持' : missing],
      ['尝试时间', projection?.attemptedAtMs ? fullTime(projection.attemptedAtMs) : missing],
      ['权限检查时间', projection?.permissionCheckedAtMs ? fullTime(projection.permissionCheckedAtMs) : missing],
      ['预览错误码', fact(projection?.errorCode)], ['预览诊断全文', fact(projection?.detail)],
      ['能力错误码', fact(capability?.errorCode)], ['能力诊断全文', fact(capability?.detail)],
      ['读取 / 操作错误', fact(error)], ['Native 请求 / 操作', missing], ['Native 原始状态', missing], ['Native API / 阶段', missing],
    ] },
  ];
  const permissionAdvice = '在系统设置 → 隐私与安全性 → 屏幕录制中允许 Luna，返回后将自动恢复。当前窗口关联和取文不会因此中断。';
  const copy = async () => {
    if (copyPending.current) return;
    const displayEpoch = copyDisplayEpoch.current;
    const text = [title, ...(authorizationOpen ? ['授权说明', permissionAdvice] : []), ...groups.flatMap(group => [group.heading, ...group.rows.map(([label, value]) => `${label}：${value}`)]), '隐私与保存', privacy].join('\n');
    copyPending.current = true; setCopying(true); setCopyStatus('');
    try { await navigator.clipboard.writeText(text); if (displayEpoch === copyDisplayEpoch.current) setCopyStatus('已复制完整详情'); }
    catch { if (displayEpoch === copyDisplayEpoch.current) setCopyStatus('复制失败，可在详情中选择文字'); }
    finally { copyPending.current = false; setCopying(false); }
  };
  return <Sheet appearance="preview" title="预览详情" description="当前状态与窗口身份 · 随预览更新" open={open} onClose={onClose} dialogId={dialogId} previewAnchor={anchor} footer={<><span role="status">{copyStatus}</span><Button icon="copy" aria-busy={copying} onPress={() => void copy()}>复制全部</Button></>}>
    <div className={styles.previewDetails}>
      {authorizationOpen ? <section><h3>授权说明</h3><p>{permissionAdvice}</p></section> : null}
      {groups.map(group => <section key={group.heading}><h3>{group.heading}</h3><dl>{group.rows.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl></section>)}
      <section><h3>隐私与保存</h3><p>{privacy}</p></section>
    </div>
  </Sheet>;
}
