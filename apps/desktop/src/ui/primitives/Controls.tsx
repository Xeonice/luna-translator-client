import { useId, useLayoutEffect, useState } from 'react';
import type { CSSProperties, ReactNode, Ref } from 'react';
import { Button as AriaButton, Dialog, Heading, Input, Label, Modal, ModalOverlay, Switch as AriaSwitch, TextArea, TextField as AriaTextField } from 'react-aria-components';
import { Icon } from '../icons/Icon';
import type { IconName } from '../icons/registry';
import styles from './controls.module.css';

export function Button({ children, onPress, disabled = false, pending = false, intent = 'default', icon, label, title, pressed, type = 'button', buttonRef, 'aria-haspopup': hasPopup, 'aria-expanded': expanded, 'aria-controls': controls, 'aria-busy': busy }: { children?: ReactNode; onPress?: () => void; disabled?: boolean; pending?: boolean; intent?: 'default' | 'primary' | 'quiet' | 'danger'; icon?: IconName; label?: string; title?: string; pressed?: boolean; type?: 'button' | 'submit'; buttonRef?: Ref<HTMLButtonElement>; 'aria-haspopup'?: 'dialog'; 'aria-expanded'?: boolean; 'aria-controls'?: string; 'aria-busy'?: boolean }) {
  return <AriaButton ref={buttonRef} className={`${styles.button} ${styles[intent] ?? ''}`} onPress={onPress} isDisabled={disabled || pending} aria-label={label} render={props => <button {...props} aria-busy={pending || busy || undefined} title={title ?? label}/>} aria-pressed={pressed} aria-haspopup={hasPopup} aria-expanded={expanded} aria-controls={controls} aria-busy={pending || busy || undefined} type={type} data-intent={intent} data-tauri-drag-region="false">{icon ? <Icon name={icon}/> : null}{children}</AriaButton>;
}
export function TextField({ label, value, onChange, description, multiline = false, rows, required = false, type = 'text', placeholder, disabled = false }: { label: string; value: string; onChange: (value: string) => void; description?: string; multiline?: boolean; rows?: number; required?: boolean; type?: 'text' | 'password' | 'url' | 'number'; placeholder?: string; disabled?: boolean }) {
  return <AriaTextField className={styles.field} value={value} onChange={onChange} isRequired={required} isDisabled={disabled} type={type}><Label data-field-label>{label}{required ? ' *' : ''}</Label>{multiline ? <TextArea placeholder={placeholder} rows={rows}/> : <Input placeholder={placeholder} autoComplete={type === 'password' ? 'new-password' : 'off'}/>} {description ? <small>{description}</small> : null}</AriaTextField>;
}
export { Select } from './Select';
export function Switch({ label, selected, onChange, disabled = false, readOnly = false, description }: { label: string; selected: boolean; onChange: (value: boolean) => void; disabled?: boolean; readOnly?: boolean; description?: string }) {
  const descriptionId = useId();
  return <AriaSwitch aria-label={label} aria-describedby={description ? descriptionId : undefined} className={styles.switchRow} isSelected={selected} onChange={onChange} isDisabled={disabled} isReadOnly={readOnly}><span data-switch-label><strong>{label}</strong>{description ? <small id={descriptionId}>{description}</small> : null}</span><span className={styles.switchTrack}><span/></span></AriaSwitch>;
}
// CSS zoom scales fixed layout units, while innerWidth/innerHeight and client
// rectangles remain viewport CSS pixels. Browser page zoom already changes the
// layout viewport, so only the explicit root CSS zoom needs division here.
function readSheetViewport() {
  const zoom = Number.parseFloat(window.getComputedStyle(document.documentElement).zoom) || 1;
  return { width: window.innerWidth / zoom, height: window.innerHeight / zoom, zoom };
}
function useSheetViewport(open: boolean) {
  const [viewport, setViewport] = useState(readSheetViewport);
  useLayoutEffect(() => {
    if (!open) return;
    const update = () => {
      const next = readSheetViewport();
      setViewport(current => current.width === next.width && current.height === next.height && current.zoom === next.zoom ? current : next);
    };
    update();
    window.addEventListener('resize', update);
    const resize = new ResizeObserver(update);
    resize.observe(document.documentElement);
    const attributes = new MutationObserver(update);
    attributes.observe(document.documentElement, { attributes: true, attributeFilter: ['style', 'class'] });
    return () => { window.removeEventListener('resize', update); resize.disconnect(); attributes.disconnect(); };
  }, [open]);
  return viewport;
}
export function Sheet({ title, description, children, footer, open, onClose, size = 'standard', appearance = 'default', dialogId, previewAnchor }: { title: string; description?: string; children: ReactNode; footer?: ReactNode; open: boolean; onClose: () => void; size?: 'standard' | 'wide'; appearance?: 'default' | 'library' | 'preview'; dialogId?: string; previewAnchor?: { right: number; top: number } }) {
  const library = appearance === 'library';
  const preview = appearance === 'preview';
  const viewport = useSheetViewport(open);
  const viewportStyle = { '--sheet-viewport-width': `${viewport.width}px`, '--sheet-viewport-height': `${viewport.height}px` } as CSSProperties;
  // The anchor is measured in client coordinates; a root CSS zoom also scales
  // fixed-position layout units. Keep position and viewport bounds in one unit.
  let previewPosition;
  if (preview && previewAnchor) {
    const { width: viewportWidth, height: viewportHeight, zoom } = viewport;
    const width = Math.min(360, viewportWidth - 24), height = Math.min(560, viewportHeight - 24);
    previewPosition = {
      width, height,
      left: Math.max(12, Math.min(previewAnchor.right / zoom + 8, viewportWidth - width - 12)),
      top: Math.max(12, Math.min(previewAnchor.top / zoom, viewportHeight - height - 12)),
    };
  }
  return <ModalOverlay style={viewportStyle} className={`${styles.overlay} ${library ? styles.libraryOverlay : ''} ${preview ? styles.previewOverlay : ''}`} isOpen={open} onOpenChange={value => { if (!value) onClose(); }} isDismissable><Modal data-ui-surface="sheet" style={previewPosition} className={`${styles.modal} ${size === 'wide' ? styles.wide : ''} ${library ? styles.librarySheet : ''} ${preview ? styles.previewSheet : ''}`}><Dialog id={dialogId} className={styles.dialog}><header className={library ? styles.libraryHeader : preview ? styles.previewHeader : styles.modalHeader}>{library ? <><span className={styles.librarySymbol}><Icon name="message" size={22}/></span><Heading slot="title">{title}</Heading>{description ? <p>{description}</p> : null}</> : <><div><Heading slot="title">{title}</Heading>{description ? <p>{description}</p> : null}</div><Button intent="quiet" icon="x" label="关闭" onPress={onClose}/></>}</header><div className={`${styles.modalBody} ${library ? styles.libraryBody : ''} ${preview ? styles.previewBody : ''}`} data-ui-scroll="sheet-body" role={preview ? 'region' : undefined} aria-label={preview ? '可滚动的完整预览详情' : undefined} tabIndex={preview ? 0 : undefined}>{children}</div>{footer ? <footer className={library ? styles.libraryFooter : preview ? styles.previewFooter : styles.modalFooter}>{footer}</footer> : null}</Dialog></Modal></ModalOverlay>;
}
