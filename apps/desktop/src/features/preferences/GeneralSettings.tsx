import { useCallback, useEffect, useId, useRef, useState } from 'react';
import type { AppSnapshot, LoginStartupState } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import { Button, Switch } from '../../ui/primitives/Controls';
import { Icon } from '../../ui/icons/Icon';
import { Group } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';

const startupLabels = {
  not_registered: '未启用', enabled: '已启用', requires_approval: '等待系统批准',
  not_found: '当前不可用', unsupported: '当前不可用',
} as const;

/** Saved preferences, main-window navigation and OS registration have separate authorities. */
export function GeneralSettings({ store }: { snapshot: AppSnapshot; store: ApplicationStore }) {
  const [startup, setStartup] = useState<LoginStartupState | null>(null);
  const [startupPending, setStartupPending] = useState(true);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [startupDetailsOpen, setStartupDetailsOpen] = useState(false);
  const startupDetailsId = useId();
  const startupGeneration = useRef(0);
  const startupWrite = useRef(false);
  const alive = useRef(false);
  const [navigationPending, setNavigationPending] = useState<'history' | 'session' | null>(null);
  const navigationFlight = useRef(false);
  const [navigationError, setNavigationError] = useState<{ page: 'history' | 'session'; message: string } | null>(null);
  const loadStartup = useCallback(async () => {
    const generation = ++startupGeneration.current;
    setStartupPending(true); setStartupError(null);
    try {
      const result = await store.port.loginStartup();
      if (alive.current && generation === startupGeneration.current) setStartup(result);
    } catch (cause) {
      if (alive.current && generation === startupGeneration.current) setStartupError(errorMessage(cause));
    } finally {
      if (alive.current && generation === startupGeneration.current) setStartupPending(false);
    }
  }, [store]);
  useEffect(() => {
    alive.current = true; void loadStartup();
    return () => { alive.current = false; startupGeneration.current += 1; };
  }, [loadStartup]);
  const changeStartup = async (enabled: boolean) => {
    if (startupPending || startupWrite.current || !startup?.canChange) return;
    const generation = ++startupGeneration.current;
    startupWrite.current = true;
    setStartupPending(true); setStartupError(null);
    try {
      const receipt = await store.port.setLoginStartup(enabled);
      if (!alive.current || generation !== startupGeneration.current) return;
      setStartup(receipt);
      // A write intention is never rendered as success. Confirm system authority.
      const confirmed = await store.port.loginStartup();
      if (!alive.current || generation !== startupGeneration.current) return;
      setStartup(confirmed);
      if (receipt.errorCode) setStartupError(receipt.detail);
    } catch (cause) {
      if (alive.current && generation === startupGeneration.current) setStartupError(errorMessage(cause));
    } finally {
      startupWrite.current = false;
      if (alive.current && generation === startupGeneration.current) setStartupPending(false);
    }
  };
  const openPage = async (page: 'history' | 'session') => {
    if (navigationFlight.current) return;
    navigationFlight.current = true; setNavigationPending(page); setNavigationError(null);
    try { await store.port.openMainPage(page); }
    catch (cause) { if (alive.current) setNavigationError({ page, message: errorMessage(cause) }); }
    finally { navigationFlight.current = false; if (alive.current) setNavigationPending(null); }
  };
  const startupNeedsAttention = Boolean(startupError || startup?.errorCode || !startup || startup.status === 'not_found' || startup.status === 'unsupported' || startup.status === 'requires_approval');
  const detailLabel = startupNeedsAttention ? '原因' : '详情';
  const startupStatus = startupPending
    ? startup ? `正在核对，上次确认：${startupLabels[startup.status]}` : '正在核对系统状态'
    : startupError ? '当前状态待确认' : startup ? startupLabels[startup.status] : '当前状态未知';
  return <>
    <Group title="阅读记录">
      <div className={styles.generalUtilityRow} data-history-policy="always">
        <div><span>始终保存原文、译文与来源</span><p>按游戏与会话回看已保存内容。</p></div>
        <div className={styles.generalTextAction}><Button intent="quiet" pending={navigationPending === 'history'} disabled={navigationPending !== null} onPress={() => void openPage('history')}>阅读历史<Icon name="external-link" size={13}/></Button></div>
      </div>
      {navigationError?.page === 'history' ? <p className={styles.generalInlineError} role="alert">阅读历史打开失败：{navigationError.message}</p> : null}
    </Group>
    <p className={styles.generalSessionNote}>暂停输入与自动翻译在<Button intent="quiet" pending={navigationPending === 'session'} disabled={navigationPending !== null} onPress={() => void openPage('session')}>当前会话</Button>中设置。{navigationError?.page === 'session' ? <span className={styles.generalInlineError} role="alert">当前会话打开失败：{navigationError.message}</span> : null}</p>
    <Group title="本机偏好">
      <div className={styles.generalStartupRow}>
        <Switch label="登录后打开 Luna" selected={startup?.status === 'enabled' || startup?.status === 'requires_approval'} disabled={startupPending || !startup?.canChange} onChange={enabled => void changeStartup(enabled)}/>
        <div className={styles.generalStartupSummary}>
          <span role="status" aria-live="polite" data-login-startup-status={startup?.status ?? 'unknown'}>{startupStatus}</span>
          <Button intent="quiet" aria-expanded={startupDetailsOpen} aria-controls={startupDetailsId} onPress={() => setStartupDetailsOpen(open => !open)}>{`${startupDetailsOpen ? '收起' : '查看'}${detailLabel}`}</Button>
        </div>
      </div>
      {startupDetailsOpen ? <div className={styles.generalStartupDetails} id={startupDetailsId}>
        {startup?.status === 'not_found' ? <p>系统未找到此应用的登录服务。此状态不会影响取文、翻译或阅读偏好的保存；具体原因和恢复方式尚未确认。</p> : null}
        <p>{startup ? `系统回执：${startup.detail}` : '尚未取得系统回执。'}</p>
        {startup?.errorCode ? <p>错误代码：<code>{startup.errorCode}</code></p> : null}
        {startupError ? <p className={styles.generalInlineError} role="alert">登录启动状态未完成核对：{startupError}</p> : null}
        <div className={styles.generalDetailActions}><Button disabled={startupPending} onPress={() => void loadStartup()}>重新读取状态</Button></div>
      </div> : null}
    </Group>
  </>;
}
