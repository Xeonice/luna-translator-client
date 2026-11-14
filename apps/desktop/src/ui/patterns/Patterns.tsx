import type { ReactNode } from 'react';
import { Icon } from '../icons/Icon';
import type { IconName } from '../icons/registry';
import styles from './patterns.module.css';

export function PageHeader({ title, description, actions }: { title: string; description?: string; actions?: ReactNode }) { return <header className={styles.pageHeader}><div><h1>{title}</h1>{description ? <p>{description}</p> : null}</div>{actions ? <div className={styles.actions}>{actions}</div> : null}</header>; }
export function EmptyState({ title, description, children, icon = 'message' }: { title: string; description: string; children?: ReactNode; icon?: IconName }) { return <section className={styles.empty}><span className={styles.emptyIcon}><Icon name={icon} size={30}/></span><h2>{title}</h2><p>{description}</p>{children ? <div className={styles.actions}>{children}</div> : null}</section>; }
export function Group({ title, children, note }: { title: string; children: ReactNode; note?: string }) { return <section className={styles.group}><h2>{title}</h2><div className={styles.groupBody}>{children}</div>{note ? <p className={styles.note}>{note}</p> : null}</section>; }
export function FormRow({ label, children, note }: { label: string; children: ReactNode; note?: string }) { return <div className={styles.formRow} data-form-row><div><strong>{label}</strong>{note ? <small>{note}</small> : null}</div><div>{children}</div></div>; }
export function Notice({ children, error = false }: { children: ReactNode; error?: boolean }) { return <div className={`${styles.notice} ${error ? styles.error : ''}`} role={error ? 'alert' : 'status'}><Icon name={error ? 'alert' : 'message'}/><span>{children}</span></div>; }
export function StatusLabel({ children }: { children: ReactNode }) { return <span className={styles.status}>{children}</span>; }
