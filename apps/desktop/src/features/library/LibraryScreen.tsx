import { cloneElement, useEffect, useId, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import type { AppSnapshot, ExecutableSelection, Game, GameInput, GameBatchItem, GameCoverResult } from '../../bridge/contracts.generated';
import type { ApplicationStore } from '../../bridge/projections';
import { useAction } from '../../bridge/projections';
import { errorMessage } from '../../bridge/application-port';
import type { ApplicationPort } from '../../bridge/application-port';
import { Icon } from '../../ui/icons/Icon';
import { Button, Select, Sheet, TextField } from '../../ui/primitives/Controls';
import { EmptyState, Notice, PageHeader, StatusLabel } from '../../ui/patterns/Patterns';
import styles from './styles.module.css';


type CoverCache = { active: number; waiting: (() => void)[]; reads: Map<string, Promise<GameCoverResult>> };
const coverCaches = new WeakMap<ApplicationPort, CoverCache>();
function readCover(port: ApplicationPort, gameId: string, assetRef: string | null) {
  let cache = coverCaches.get(port);
  if (!cache) { cache = { active: 0, waiting: [], reads: new Map() }; coverCaches.set(port, cache); }
  const owner = cache;
  const key = JSON.stringify([gameId, assetRef]);
  const known = owner.reads.get(key);
  if (known) return known;
  const read = (async () => {
    await new Promise<void>(resolve => {
      const run = () => { owner.active += 1; resolve(); };
      if (owner.active < 2) run(); else owner.waiting.push(run);
    });
    try {
      const result = await port.readGameCover({ gameId, expectedAssetRef: assetRef });
      if (result.gameId !== gameId || result.status === 'ready' && assetRef !== null && result.assetRef !== assetRef) throw new Error('封面归属或引用不匹配');
      return result;
    } finally { owner.active -= 1; owner.waiting.shift()?.(); }
  })();
  owner.reads.set(key, read);
  // Bounded bytes, keyed by the immutable asset reference, never by subtitle updates.
  while (owner.reads.size > 16) owner.reads.delete(owner.reads.keys().next().value!);
  void read.catch(() => { if (owner.reads.get(key) === read) owner.reads.delete(key); });
  return read;
}
function GameCover({ game, port }: { game: Game; port: ApplicationPort }) {
  const [cover, setCover] = useState<{ url: string | null; status: string; loaded: boolean; origin: GameCoverResult['origin']; errorCode: string | null }>({ url: null, status: '正在读取本机封面', loaded: false, origin: null, errorCode: null });
  useEffect(() => {
    let current = true;
    let objectUrl: string | null = null;
    setCover({ url: null, status: '正在读取本机封面', loaded: false, origin: null, errorCode: null });
    void readCover(port, game.gameId, game.coverAssetRef).then(result => {
      if (!current) return;
      if (result.status !== 'ready' || result.mimeType !== 'image/jpeg' || !result.payload?.length || result.payload.length > 524288) {
        setCover({ url: null, status: result.status === 'missing' ? '尚无封面 · 连接后自动获取游戏画面' : '封面文件暂不可用，取文可继续', loaded: false, origin: null, errorCode: result.errorCode });
        return;
      }
      objectUrl = URL.createObjectURL(new Blob([new Uint8Array(result.payload)], { type: result.mimeType }));
      setCover({ url: objectUrl, status: '封面已读取', loaded: false, origin: result.origin, errorCode: null });
    }).catch(() => { if (current) setCover({ url: null, status: '封面读取失败', loaded: false, origin: null, errorCode: null }); });
    return () => { current = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [port, game.gameId, game.coverAssetRef]);
  const originLabel = cover.origin === 'steam_library' ? '本机 Steam 封面' : cover.origin === 'game_resource' ? '本机游戏资源' : cover.origin === 'session_frame' ? '已关联游戏的首张画面' : cover.status;
  return <div className={styles.cover} title={`${originLabel}${cover.errorCode ? ` · ${cover.errorCode}` : ''}`} data-cover-source={cover.origin ?? undefined} data-cover-state={cover.url ? cover.loaded ? 'ready' : 'loading' : 'placeholder'}>
    {cover.url ? <img src={cover.url} alt={`${game.displayName} 封面`} aria-description={originLabel} onLoad={() => setCover(current => current.url === cover.url ? { ...current, loaded: true } : current)} onError={() => { if (cover.url) URL.revokeObjectURL(cover.url); setCover(current => current.url === cover.url ? { url: null, status: '封面图像无法显示', loaded: false, origin: null, errorCode: null } : current); }}/> : <small className={styles.coverStatus}>{cover.status}</small>}
    <strong>{game.displayName}</strong>
  </div>;
}

const pathLeaf = (value: string | null) => value?.split(/[\\/]/).filter(Boolean).at(-1);
function environmentLabel(game: Game) { const label = game.target.environmentKind === 'crossover' ? 'CrossOver' : game.target.environmentKind === 'native_mac' ? 'macOS' : game.target.environmentKind; return `${label} · ${pathLeaf(game.target.bottleLocator) ?? '环境待核对'}`; }

function gameInput(game: Game | null): GameInput {
  return game ? { gameId: game.gameId, rowRevision: game.rowRevision, displayName: game.displayName, originalTitle: game.originalTitle, sourceLanguage: game.sourceLanguage, coverAssetRef: game.coverAssetRef, target: { ...game.target, arguments: [...game.target.arguments] } } : { gameId: null, rowRevision: null, displayName: '', originalTitle: null, sourceLanguage: null, coverAssetRef: null, target: { environmentKind: 'unresolved', environmentKey: '', bottleLocator: null, launchPath: null, canonicalExePath: null, targetPathKey: null, workingDirectory: null, arguments: [], architecture: null, verificationState: 'unresolved' } };
}
function MetadataField({ label, value, onChange, children, description, placeholder, required = false }: { label: string; value?: string; onChange?: (value: string) => void; children?: ReactElement<{ id?: string; 'aria-describedby'?: string }>; description?: string; placeholder?: string; required?: boolean }) {
  const id = useId();
  const descriptionId = description ? `${id}-description` : undefined;
  return <div className={styles.metadataField}><label htmlFor={id}>{label}{required ? <span className={styles.required} aria-hidden="true"> · 必填</span> : null}</label><div>{children ? cloneElement(children, { id, 'aria-describedby': descriptionId }) : <input id={id} value={value ?? ''} onChange={event => onChange?.(event.target.value)} placeholder={placeholder} required={required} aria-describedby={descriptionId}/>} {description ? <small id={descriptionId}>{description}</small> : null}</div></div>;
}

export function LibraryScreen({ snapshot, store, onConnect, onActivate, active = true }: { snapshot: AppSnapshot; store: ApplicationStore; onConnect: (game: Game | null) => void; onActivate: () => void; active?: boolean }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [editor, setEditor] = useState<{ items: GameBatchItem[]; selectedItemId: string } | null>(null);
  const editing = editor?.items.find(item => item.itemId === editor.selectedItemId)?.game ?? null;
  const [saveNotice, setSaveNotice] = useState<string | null>(null);
  const setEditing = (value: GameInput | null | ((current: GameInput | null) => GameInput | null)) => {
    if (value === null) { setEditor(null); return; }
    if (typeof value !== 'function') { const itemId = crypto.randomUUID(); setEditor({ items: [{ itemId, game: value }], selectedItemId: itemId }); return; }
    setEditor(current => {
      if (!current) return current;
      const selected = current.items.find(item => item.itemId === current.selectedItemId)?.game ?? null;
      const next = value(selected);
      if (!next) return null;
      return { ...current, items: current.items.map(item => item.itemId === current.selectedItemId ? { ...item, game: next } : item) };
    });
  };
  const [archiving, setArchiving] = useState<Game | null>(null);
  const [dropError, setDropError] = useState<string | null>(null);
  const importPrograms = (programs: ExecutableSelection[]) => {
    if (!programs.length) return;
    const items = programs.map(program => { const value = gameInput(null); return { itemId: crypto.randomUUID(), game: { ...value, displayName: program.displayName, target: { ...value.target, environmentKind: program.bottleLocator ? 'crossover' : 'unresolved', launchPath: program.path, architecture: program.architecture, environmentKey: program.environmentKey ?? '', bottleLocator: program.bottleLocator } } }; });
    setEditor({ items, selectedItemId: items[0].itemId }); setSaveNotice(null); setDropError(null); onActivate();
  };
  const dropHandler = useRef<(paths: string[]) => void>(() => {});
  dropHandler.current = paths => {
    if (!paths.length || action.pending) return;
    const emptyNewDraft = editor?.items.length === 1 && JSON.stringify(editor.items[0].game) === JSON.stringify(gameInput(null));
    if (editor && !emptyNewDraft) {
      setDropError('当前资料尚未保存。请先保存或取消本次编辑，再拖入新的程序；现有资料已保留。');
      return;
    }
    setDropError(null);
    void action.run(async () => { importPrograms(await store.port.inspectExecutables(paths)); });
  };
  useEffect(() => { let active = true; let stop: (() => void) | undefined; void store.port.onExecutableDrop(paths => { if (active) dropHandler.current(paths); }).then(unlisten => { if (active) stop = unlisten; else unlisten(); }).catch(error => { if (active) setDropError(errorMessage(error)); }); return () => { active = false; stop?.(); }; }, [store]);
  const action = useAction();
  const selected = snapshot.games.find(game => game.gameId === selectedId) ?? snapshot.games[0] ?? null;
  const filtered = snapshot.games.filter(game => `${game.displayName} ${game.originalTitle ?? ''}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()));
  const edit = (value: Partial<GameInput>) => setEditing(current => current ? { ...current, ...value } : current);
  const target = (value: Partial<GameInput['target']>) => setEditing(current => current ? { ...current, target: { ...current.target, ...value } } : current);
  const openAdd = () => { action.clearError(); setSaveNotice(null); setDropError(null); const itemId = crypto.randomUUID(); setEditor({ items: [{ itemId, game: gameInput(null) }], selectedItemId: itemId }); };
  const browsePrograms = () => void action.run(async () => {
    const programs = await store.port.pickExecutables();
    if (editing?.gameId) {
      if (programs.length > 1) throw new Error('编辑现有资料时请只选择一个程序；批量程序请使用添加游戏。');
      const program = programs[0];
      if (program) setEditing(current => current ? { ...current, target: { ...current.target, launchPath: program.path, architecture: program.architecture, environmentKey: program.environmentKey ?? current.target.environmentKey, bottleLocator: program.bottleLocator } } : current);
    } else importPrograms(programs);
  });
  const saveEditor = async () => {
    if (!editor || !editing) return;
    if (editing.gameId) { await store.command({ kind: 'save_game', payload: editing }); setSaveNotice('游戏资料已保存'); }
    else {
      const receipt = await store.command({ kind: 'save_games', payload: { items: editor.items } });
      const expected = new Set(editor.items.map(item => item.itemId));
      const results = receipt.gameResults ?? [];
      if (results.length !== editor.items.length || new Set(results.map(item => item.itemId)).size !== expected.size || results.some(item => !expected.has(item.itemId) || !item.gameId || item.rowRevision < 1)) throw new Error('保存结果无法对应全部待添加程序，请核对游戏库。当前资料仍然保留。');
      const reused = results.filter(item => item.reused).length;
      setSaveNotice(`${results.length} 项已保存${reused ? `，其中 ${reused} 项复用已有记录` : ''}`);
    }
    setEditor(null);
  };
  return <><div className={styles.libraryContent} data-ui-scroll="main-content"><PageHeader title="游戏库" description="添加游戏，或从已经打开的游戏窗口开始。" actions={<><Button onPress={() => onConnect(snapshot.games.find(game => game.gameId === selectedId) ?? null)}>连接运行中的游戏…</Button><Button intent="primary" onPress={openAdd}>添加游戏…</Button></>}/>{saveNotice ? <Notice>{saveNotice}</Notice> : null}{dropError && !editing ? <Notice error>{dropError}</Notice> : null}{action.error && !editing && !archiving ? <Notice error>{action.error}</Notice> : null}{snapshot.games.length === 0 ? <section className={styles.empty}><span className={styles.emptySymbol}><Icon name="message" size={36}/></span><h2>把第一款游戏加入这里</h2><p>选择游戏程序保存到库，或先选择运行中的游戏。选择窗口或进程，核对后确认“关联并接入”，自动启动文本接入。</p><div><Button intent="primary" onPress={openAdd}>添加游戏…</Button><Button onPress={() => onConnect(null)}>选择运行中的游戏…</Button></div></section> : <div className={styles.body}><div className={styles.toolbar}><label className={styles.searchControl}><Icon name="search" size={13}/><input aria-label="搜索游戏" value={search} onChange={event => setSearch(event.target.value)} placeholder="搜索游戏"/></label><span>{snapshot.games.length} 款游戏</span></div><div className={styles.grid}>{filtered.map(game => <button className={`${styles.gameCard} ${selected?.gameId === game.gameId ? styles.selected : ''}`} key={game.gameId} aria-pressed={selected?.gameId === game.gameId} onClick={() => setSelectedId(game.gameId)}><GameCover game={game} port={store.port}/><div className={styles.cardBody}><strong>{game.displayName}</strong><small>{game.originalTitle ?? game.target.environmentKind}</small><StatusLabel>{snapshot.session?.gameId === game.gameId && ['connected', 'active'].includes(snapshot.session.state) ? '已连接' : `${game.target.architecture ?? '位数待核实'} · ${game.sourceLanguage === 'ja' ? '日语' : game.sourceLanguage === 'zh-CN' ? '中文' : '语言待确认'}`}</StatusLabel></div></button>)}</div>{filtered.length === 0 ? <EmptyState title="没有匹配的游戏" description="试试原名，或清空搜索。" icon="search"/> : null}{selected ? <section className={styles.details}><header><div><h2>{selected.displayName}</h2><p>{selected.originalTitle ?? '游戏资料'}</p></div><div className={styles.actions}><Button onPress={() => { action.clearError(); setEditing(gameInput(selected)); }}>编辑…</Button><Button onPress={() => setArchiving(selected)}>移出游戏库…</Button></div></header><div className={styles.detailFields}><div><small>运行环境</small><span>{environmentLabel(selected)}</span></div><div><small>实际取文程序</small><span><span className={styles.code}>{pathLeaf(selected.target.canonicalExePath) ?? '待选择'}</span> · {selected.target.architecture ?? '待核实'}</span></div><div className={styles.wide}><small>目标路径</small><span className={styles.code}>{selected.target.canonicalExePath ?? '连接时核对实际游戏进程'}</span></div><div className={`${styles.wide} ${styles.windowBinding}`}><div><small>运行窗口</small><span>{snapshot.session?.gameId === selected.gameId ? `当前取文会话 · ${snapshot.session.state}` : '尚未关联运行窗口'}</span></div></div></div></section> : null}<p className={styles.note}>先在 CrossOver 中打开游戏，再从页头连接。窗口核对后一次确认即启动 Hook 接入；收到候选后再选择正文。解除关联保留取文，断开取文使用独立操作。原生 macOS 目前仅可建库。</p></div>}</div>
    <Sheet size="wide" appearance="library" title={editing?.gameId ? '编辑游戏资料' : '添加游戏'} description="从文件添加只建立游戏记录。需要连接时，再选择已运行的目标。" open={active && editing !== null} onClose={() => { if (!action.pending) setEditing(null); }} footer={<><Button disabled={action.pending} onPress={() => setEditing(null)}>取消</Button><Button intent="primary" pending={action.pending} disabled={!editor || editor.items.some(item => !item.game.displayName.trim())} onPress={() => void action.run(saveEditor)}>{editing?.gameId ? '保存到游戏库' : '添加到游戏库'}</Button></>}>{editing ? <fieldset disabled={action.pending} className={styles.editorFieldset}><div className={styles.editor}>
      {dropError ? <Notice error>{dropError}</Notice> : null}
      <div className={styles.dropZone}><span className={styles.dropIcon}><Icon name="window" size={24}/></span><strong>选择或拖入游戏程序</strong><span>支持多个 .exe / .lnk；添加后核对路径与容器</span><Button pending={action.pending} onPress={browsePrograms}>选择文件…</Button></div>
      <div className={styles.previewHeading}><h3>{editing.gameId ? '游戏资料' : `待添加 · ${editor?.items.length ?? 0} 项`}</h3><span>保存只建立游戏记录</span></div>
      {editor && editor.items.length > 1 ? <><div className={styles.importItems} role="group" aria-label="待添加程序">{editor.items.map(item => <button type="button" key={item.itemId} aria-pressed={editor.selectedItemId === item.itemId} onClick={() => setEditor(current => current ? { ...current, selectedItemId: item.itemId } : current)}><Icon name="window"/><span>{item.game.displayName || '待命名程序'}</span><small>{item.game.target.bottleLocator?.split(/[\\/]/).filter(Boolean).at(-1) ?? '容器待核对'}</small></button>)}{editor.items.findIndex(item => item.itemId === editor.selectedItemId) < editor.items.length - 1 ? <Button intent="quiet" onPress={() => setEditor(current => { if (!current) return current; const at = current.items.findIndex(item => item.itemId === current.selectedItemId); return { ...current, selectedItemId: current.items[at + 1].itemId }; })}>核对下一项</Button> : null}</div></> : null}
      <div className={styles.metadata}>
        <MetadataField label="游戏名称" value={editing.displayName} onChange={displayName => edit({ displayName })} required/>
        <MetadataField label="原名" value={editing.originalTitle ?? ''} onChange={originalTitle => edit({ originalTitle: originalTitle || null })}/>
        <MetadataField label="原文语言"><Select hideLabel label="原文语言" value={editing.sourceLanguage ?? ''} onChange={value => edit({ sourceLanguage: value || null })}><option value="ja">日语</option><option value="zh-CN">中文</option><option value="en">英语</option><option value="">待确认</option></Select></MetadataField>
        <MetadataField label="运行环境"><Select hideLabel label="运行环境" value={editing.target.environmentKind} onChange={value => target({ environmentKind: value })}><option value="crossover">CrossOver</option><option value="unresolved">待核对运行环境</option></Select></MetadataField>
        <MetadataField label="容器名称" value={editing.target.bottleLocator ?? ''} onChange={bottleLocator => target({ environmentKind: bottleLocator ? 'crossover' : 'unresolved', environmentKey: '', bottleLocator: bottleLocator || null })} placeholder="本机 Bottle 文件夹的绝对路径"/>
        <MetadataField label="程序文件（仅记录）" value={editing.target.launchPath ?? ''} onChange={launchPath => target({ launchPath: launchPath || null })} placeholder="例如 /…/Game.exe" description="仅用于识别程序来源，客户端不启动游戏。程序路径可与实际取文进程路径不同。"/>
        <MetadataField label="目标进程路径" value={editing.target.canonicalExePath ?? ''} onChange={canonicalExePath => target({ canonicalExePath: canonicalExePath || null })} placeholder="核对实际游戏进程的完整路径" description="匹配环境、容器与完整目标路径；不只比较 exe 文件名。"/>
        <MetadataField label="目标架构"><Select hideLabel label="目标架构" value={editing.target.architecture ?? ''} onChange={value => target({ architecture: value || null })}><option value="">待核对</option><option value="x86">32 位 · x86</option><option value="x64">64 位 · x64</option></Select></MetadataField>
        <div className={styles.metadataField}><span>启动参数</span><div><small>保留原有游戏记录的参数信息；客户端只连接已运行的游戏。</small>{editing.target.arguments.map((argument, index) => <div className={styles.argument} key={index}><TextField label={`启动参数 ${index + 1}`} value={argument} onChange={value => target({ arguments: editing.target.arguments.map((current, at) => at === index ? value : current) })}/><Button intent="quiet" label={`移除启动参数 ${index + 1}`} onPress={() => target({ arguments: editing.target.arguments.filter((_, at) => at !== index) })}>移除</Button></div>)}<Button onPress={() => target({ arguments: [...editing.target.arguments, ''] })}>添加启动参数</Button></div></div>
        <MetadataField label="工作目录" value={editing.target.workingDirectory ?? ''} onChange={workingDirectory => target({ workingDirectory: workingDirectory || null })} placeholder="可留空，使用程序所在目录"/>
      </div>
      {action.error ? <Notice error>{action.error}</Notice> : null}
    </div></fieldset> : null}</Sheet>
    <Sheet title="移出游戏库？" description={`${archiving?.displayName ?? ''} 将从游戏库中移出。已有历史和游戏文件保留，当前取文会话不会偷偷切换。`} open={active && archiving !== null} onClose={() => setArchiving(null)} footer={<><Button onPress={() => setArchiving(null)}>取消</Button><Button intent="danger" pending={action.pending} onPress={() => { if (archiving) void action.run(async () => { await store.command({ kind: 'archive_game', payload: { gameId: archiving.gameId, rowRevision: archiving.rowRevision } }); setArchiving(null); }); }}>移出游戏库</Button></>}>{action.error ? <Notice error>{action.error}</Notice> : null}</Sheet>
  </>;
}
