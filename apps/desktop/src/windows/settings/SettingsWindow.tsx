import { useEffect, useRef, useState } from 'react';
import type { SettingsPage } from '../../app/navigation';
import { errorMessage } from '../../bridge/application-port';
import type { ApplicationStore } from '../../bridge/projections';
import { useApplication, useAction } from '../../bridge/projections';
import { WindowFrame, WindowLoading } from '../../ui/patterns/WindowFrame';
import { PageHeader, FormRow, Group, Notice } from '../../ui/patterns/Patterns';
import { Button, Sheet } from '../../ui/primitives/Controls';
import { Icon } from '../../ui/icons/Icon';
import type { IconName } from '../../ui/icons/registry';
import { ModelConfigScreen } from '../../features/model-config';
import { PreferencesScreen } from '../../features/preferences';
import { DiagnosticsScreen } from '../../features/diagnostics';
import styles from './styles.module.css';

const destinations: { id: SettingsPage; label: string; icon: IconName; description: string }[] = [
  { id: 'general', label: '通用', icon: 'settings', description: '阅读偏好与本机设置。' },
  { id: 'capture', label: '输入与HOOK', icon: 'plug', description: '选择游戏目标、接入与正文来源。' },
  { id: 'ocr', label: 'OCR引擎', icon: 'scan', description: '屏幕文字识别的产品范围。' },
  { id: 'translation', label: '翻译服务', icon: 'languages', description: '添加和编辑服务，按需配置阅读用途。' },
  { id: 'processing', label: '文本处理', icon: 'sliders', description: '原文与阅读正文的关系。' },
  { id: 'speech', label: '语音', icon: 'volume', description: '朗读与语音能力的产品范围。' },
  { id: 'display', label: '显示与快捷键', icon: 'type', description: '注音、阅读尺寸、材质与快捷键。' },
  { id: 'network', label: '网络服务', icon: 'globe', description: '翻译连接的网络策略与后续网络能力。' },
  { id: 'data', label: '配置与数据', icon: 'database', description: '本机配置状态、阅读记录与内置资源。' },
  { id: 'diagnostics', label: '更新与诊断', icon: 'activity', description: '查看当前运行状态与可恢复的问题。' },
];

export function SettingsWindow({ store }: { store: ApplicationStore }) {
  const { snapshot, error } = useApplication(store);
  const [page, setPage] = useState<SettingsPage>('translation');
  const [search, setSearch] = useState('');
  const [serviceDirty, setServiceDirty] = useState(false), [pendingPage, setPendingPage] = useState<SettingsPage | 'main' | null>(null), [serviceReset, setServiceReset] = useState(0);
  const [modelRoute, setModelRoute] = useState<{ section: 'connection'; sequence: number } | null>(null);
  const [navigationError, setNavigationError] = useState<string | null>(null), [navigationAttempt, setNavigationAttempt] = useState(0);
  const navigation = useRef({ store, revision: 0 });
  const action = useAction();
  const destination = destinations.find(item => item.id === page)!;
  const go = (destination: SettingsPage | 'main') => { if (destination === page) { setPendingPage(null); return; } if (page === 'translation' && serviceDirty) { setPendingPage(destination); return; } if (destination === 'main') void action.run(async () => { await store.port.openWindow('main'); await store.port.closeWindow(); }); else setPage(destination); };
  const currentGo = useRef(go); currentGo.current = go;
  useEffect(() => {
    let active = true; let dispose: (() => void) | undefined;
    if (navigation.current.store !== store) navigation.current = { store, revision: 0 };
    void store.port.onSettingsPageRequested(request => {
      if (!active || request.revision <= navigation.current.revision) return;
      navigation.current.revision = request.revision;
      setNavigationError(null); setSearch(''); currentGo.current(request.intent);
    }).then(unsubscribe => {
      if (active) { dispose = unsubscribe; setNavigationError(null); }
      else unsubscribe();
    }).catch(reason => { if (active) setNavigationError(errorMessage(reason)); });
    return () => { active = false; dispose?.(); };
  }, [store, navigationAttempt]);
  const confirmLeave = () => { const destination = pendingPage; setPendingPage(null); setServiceReset(value => value + 1); setServiceDirty(false); if (destination === 'main') void action.run(async () => { await store.port.openWindow('main'); await store.port.closeWindow(); }); else if (destination) setPage(destination); };
  const openConnections = () => { setPage('translation'); setModelRoute(current => ({ section: 'connection', sequence: (current?.sequence ?? 0) + 1 })); };
  const preferencesSection = page === 'data' ? 'resources' : page === 'general' ? 'general' : 'display';
  return <WindowFrame title="设置" visualVariant={page === 'general' ? 'general-settings' : undefined}>{snapshot ? <div className={styles.layout} data-client-container="settings-a1000x720" data-settings-page={page}>
    <aside className={styles.sidebar} data-platform-scrollbar={page === 'general' ? true : undefined}>
      <button className={styles.lightReturn} onClick={() => go('main')}><Icon name="arrow-left"/><span>返回主界面</span></button>
      <label className={styles.search}><Icon name="search"/><input type="search" aria-label="搜索设置" placeholder="搜索设置" value={search} onChange={event => setSearch(event.target.value)}/></label>
      <nav className={styles.navigation} aria-label="设置分类">{destinations.filter(item => item.label.includes(search.trim())).map(item => <button key={item.id} className={`${styles.item} ${page === item.id ? styles.selected : ''}`} onClick={() => go(item.id)} aria-current={page === item.id ? 'page' : undefined}><Icon name={item.icon}/><span>{item.label}</span></button>)}</nav>
      <div className={styles.footer}>个人配置 · 默认<br/><span>全局与游戏覆盖</span></div>
    </aside>
    <main className={styles.content}>
      {action.error || error ? <div className={styles.syncError}><Notice error>{action.error ?? error}</Notice></div> : null}
      {navigationError ? <div className={styles.syncError}><Notice error>设置导航连接失败：{navigationError}<Button onPress={() => setNavigationAttempt(value => value + 1)}>重新连接设置导航</Button></Notice></div> : null}
      <div className={styles.featurePanel} hidden={page !== 'diagnostics'}><DiagnosticsScreen snapshot={snapshot} store={store}/></div>
      <div className={styles.featurePanel} hidden={page === 'diagnostics'}>
        {page !== 'translation' && page !== 'display' && page !== 'general' ? <PageHeader title={destination.label} description={destination.description}/> : null}
        <div className={styles.featurePanel} hidden={page !== 'translation'}><ModelConfigScreen snapshot={snapshot} store={store} active={page === 'translation'} resetKey={serviceReset} onDirtyChange={setServiceDirty} routeIntent={modelRoute} header={<header className={styles.sectionHeading}><span className={styles.headingIcon}><Icon name="languages" size={24}/></span><div><h1>翻译服务</h1><p>选择你要使用的服务，再填写必要的信息。</p></div></header>}/></div>
        <div className={styles.featurePanel} hidden={page !== 'capture'}><div className={styles.scroll}>
          <Group title="游戏取文"><FormRow label="当前会话"><span>{snapshot.session?.state ?? '尚未连接'}</span></FormRow><FormRow label="输入方式"><span>LunaHook · CrossOver</span></FormRow><FormRow label="窗口关联" note="一次“关联并接入”确认启动文本接入"><Button onPress={() => void action.run(() => store.port.openWindow('main'))}>返回游戏库选择目标</Button></FormRow></Group>
          <Notice>选择窗口仅预览；收到候选后再选择正文。暂停、解除关联与断开取文分别操作。</Notice>
        </div></div>
        <div className={styles.featurePanel} hidden={page !== 'ocr'}><div className={styles.scroll}><Group title="阶段范围"><FormRow label="当前可用"><span>游戏取文与手动文本输入。本期窗口预览独立于 OCR，按系统需要申请录屏权限。</span></FormRow><FormRow label="后续范围"><span>OCR 文字识别属于后续范围；本版不提供启动入口。</span></FormRow></Group></div></div>
        <div className={styles.featurePanel} hidden={page !== 'speech'}><div className={styles.scroll}><Group title="阶段范围"><FormRow label="后续范围"><span>朗读与语音模型属于后续范围。当前可使用文字原文、译文与内置词典。</span></FormRow></Group></div></div>
        <div className={styles.featurePanel} hidden={page !== 'processing'}><div className={styles.scroll}><Group title="取文与正文"><FormRow label="原文保留"><span>原始取文与阅读正文分别保留，阅读记录保留来源。</span></FormRow><FormRow label="翻译输入"><span>注音与词性标记只用于显示，不加入原文。</span></FormRow></Group><Group title="阶段范围"><FormRow label="高级替换与脚本"><span>属于后续范围，本版不执行用户脚本。</span></FormRow></Group></div></div>
        <div className={styles.featurePanel} hidden={page !== 'network'}><div className={styles.scroll}><Group title="翻译连接"><FormRow label="网络策略"><span>在每个服务连接中设置系统代理继承、直接连接或独立代理。</span></FormRow><FormRow label="连接设置"><Button onPress={openConnections}>前往连接设置</Button></FormRow></Group><Group title="阶段范围"><FormRow label="Web API发布"><span>属于后续范围，本版不提供对外服务启动入口。</span></FormRow></Group></div></div>
        <div className={styles.featurePanel} hidden={!['general', 'display', 'data'].includes(page)}>
          {page === 'data' ? <div className={styles.dataSummary}><Group title="当前配置档"><FormRow label="配置状态"><span>草稿 v{snapshot.configuration.draftHeadRevision} · 已应用 v{snapshot.configuration.appliedHeadRevision}</span></FormRow><FormRow label="配置管理"><Button onPress={() => setPage('translation')}>前往翻译服务</Button></FormRow></Group></div> : null}
          <PreferencesScreen snapshot={snapshot} store={store} section={preferencesSection}/>
        </div>
      </div>
    </main>
    <Sheet title="离开前放弃未保存的编辑？" description="已保存服务和当前选择保持不变；未保存的密钥会清除。" open={pendingPage !== null} onClose={() => setPendingPage(null)} footer={<><Button onPress={() => setPendingPage(null)}>继续编辑</Button><Button intent="danger" onPress={confirmLeave}>放弃并离开</Button></>}><p>当前服务页有未保存的更改。</p></Sheet>
  </div> : <WindowLoading error={error} onRetry={() => void store.refresh()}/>}</WindowFrame>;
}
