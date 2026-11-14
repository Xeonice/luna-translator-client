import type { ReactNode } from 'react';
import { Button } from '../primitives/Controls';
import styles from './window.module.css';

export function WindowFrame({ title, children, tools, compact = false, visualVariant }: { title: string; children: ReactNode; tools?: ReactNode; compact?: boolean; visualVariant?: 'general-settings' }) { return <div className={`${styles.frame} ${compact ? styles.compact : ''}${visualVariant === 'general-settings' ? ` ${styles.generalSettings}` : ''}`}><header className={styles.titlebar} data-tauri-drag-region><div className={styles.nativeControlsSpace} data-tauri-drag-region/><strong data-tauri-drag-region>{title}</strong><div className={styles.tools}>{tools}</div></header>{children}</div>; }
export function WindowLoading({ error, onRetry }: { error: string | null; onRetry: () => void }) { return <div className={styles.loading}><strong>{error ? '暂时无法读取应用状态' : '正在打开阅读空间…'}</strong>{error ? <><p role="alert">{error}</p><Button onPress={onRetry}>重新同步</Button></> : <p role="status">正在连接本机应用核心</p>}</div>; }
