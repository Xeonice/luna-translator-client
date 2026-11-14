import type { ReactNode } from 'react';
import { useId } from 'react';
import styles from './styles.module.css';

/** Configuration forms keep help with the value it explains, including at 200%. */
export function ConfigurationRow({ label, children, note }: { label: string; children: ReactNode; note?: ReactNode }) {
  return <div className={styles.formRow} data-form-row><div className={styles.rowLabel}>{label}</div><div className={styles.rowValue}>{children}{note ? <div className={styles.help}>{note}</div> : null}</div></div>;
}

/** A compact checkbox appearance with the existing binary-switch semantics. */
export function ConfigurationSwitch({ label, selected, onChange, description, disabled = false }: { label: string; selected: boolean; onChange: (selected: boolean) => void; description?: string; disabled?: boolean }) {
  const id = useId();
  return <><label className={styles.check}><input type="checkbox" role="switch" checked={selected} disabled={disabled} aria-describedby={description ? id : undefined} onChange={event => onChange(event.target.checked)}/><span>{label}</span></label>{description ? <div id={id} className={styles.help}>{description}</div> : null}</>;
}
