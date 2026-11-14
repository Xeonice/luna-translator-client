import { Children, Fragment, isValidElement, useLayoutEffect, useRef, useState } from 'react';
import type { ComponentProps, CSSProperties, ReactNode, RefObject } from 'react';
import { Button, Label, ListBox, ListBoxItem, Popover, Select as AriaSelect, SelectValue } from 'react-aria-components';
import { Icon } from '../icons/Icon';
import styles from './controls.module.css';

export interface SelectOption { value: string; label: string; disabled?: boolean }
export interface SelectProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  children?: ReactNode;
  options?: readonly SelectOption[];
  disabled?: boolean;
  state?: 'ready' | 'loading' | 'error';
  placeholder?: string;
  error?: string;
  onRetry?: () => void;
  hideLabel?: boolean;
  appearance?: 'default' | 'general-settings';
  id?: string;
  'aria-describedby'?: string;
}

function textContent(node: ReactNode): string {
  return Children.toArray(node).map(child => isValidElement<{ children?: ReactNode }>(child) ? textContent(child.props.children) : typeof child === 'string' || typeof child === 'number' ? String(child) : '').join('');
}
/** Static React options are accepted for existing callers; no DOM is scanned or restyled. */
function optionsFromChildren(children: ReactNode, inheritedDisabled = false): SelectOption[] {
  return Children.toArray(children).flatMap(child => {
    if (!isValidElement<{ value?: string | number; children?: ReactNode; disabled?: boolean }>(child)) return [];
    if (child.type === Fragment || child.type === 'optgroup') return optionsFromChildren(child.props.children, inheritedDisabled || child.props.disabled === true);
    if (child.type !== 'option') return [];
    const label = textContent(child.props.children);
    return [{ value: String(child.props.value ?? label), label, disabled: inheritedDisabled || child.props.disabled === true }];
  });
}
const optionKey = (value: string) => `value:${value}`;

function effectiveZoom(element: Element | null): number {
  let zoom = 1;
  for (let parent = element; parent; parent = parent.parentElement) {
    const value = getComputedStyle(parent).zoom;
    const factor = Number.parseFloat(value) / (value.endsWith('%') ? 100 : 1);
    if (Number.isFinite(factor) && factor > 0) zoom *= factor;
  }
  return zoom;
}

/** React Aria measures viewport pixels. Keep its positioned portal in that
 * coordinate space; restore the trigger's CSS zoom on the list content only. */
function useSelectPortal() {
  const field = useRef<HTMLDivElement>(null);
  const [isOpen, setOpen] = useState(false);
  const [portal, setPortal] = useState<{ container: HTMLDivElement; scale: number } | null>(null);
  const onOpenChange = (next: boolean) => {
    if (next && !isOpen && field.current) {
      const document = field.current.ownerDocument;
      const container = document.createElement('div');
      container.className = styles.selectPortal;
      container.dataset.selectPortal = '';
      const scale = effectiveZoom(field.current);
      container.style.zoom = String(1 / effectiveZoom(document.body));
      container.style.setProperty('--select-overlay-scale', String(scale));
      document.body.append(container);
      setPortal({ container, scale });
    }
    setOpen(next);
  };
  useLayoutEffect(() => {
    if (!portal) return;
    const { container } = portal;
    if (!isOpen) { container.remove(); setPortal(null); return; }
    if (!container.isConnected) container.ownerDocument.body.append(container);
    const update = () => {
      if (!field.current) return;
      const scale = effectiveZoom(field.current);
      container.style.zoom = String(1 / effectiveZoom(container.ownerDocument.body));
      container.style.setProperty('--select-overlay-scale', String(scale));
      setPortal(current => current?.container === container && current.scale !== scale ? { container, scale } : current);
    };
    const view = container.ownerDocument.defaultView;
    view?.addEventListener('resize', update);
    view?.visualViewport?.addEventListener('resize', update);
    const observer = new MutationObserver(update);
    for (let parent: Element | null = field.current; parent; parent = parent.parentElement) observer.observe(parent, { attributes: true, attributeFilter: ['style', 'class'] });
    return () => { observer.disconnect(); view?.removeEventListener('resize', update); view?.visualViewport?.removeEventListener('resize', update); container.remove(); };
  }, [isOpen, portal?.container]);
  return { field, isOpen, onOpenChange, container: portal?.container, scale: portal?.scale ?? 1 };
}

/** The General source keeps fractional viewport coordinates. React Aria still
 * owns selection, dismissal and focus; this opt-in measures presentation only. */
function useGeneralSelectPosition(enabled: boolean, field: RefObject<HTMLDivElement | null>, open: boolean, container: HTMLDivElement | undefined, scale: number) {
  const panel = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<CSSProperties>();
  useLayoutEffect(() => {
    if (!enabled || !open || !container || !field.current || !panel.current) return;
    const trigger = field.current.querySelector<HTMLElement>('[role="combobox"]');
    const overlay = panel.current;
    const view = overlay.ownerDocument.defaultView;
    if (!trigger || !view) return;
    const update = () => {
      const rect = trigger.getBoundingClientRect();
      const safe = 12;
      const width = Math.min(Math.max(rect.width, 220 * scale), view.innerWidth - safe * 2);
      const maxHeight = (view.innerHeight - safe * 2) / scale;
      overlay.style.zoom = String(scale);
      overlay.style.width = `${width / scale}px`;
      overlay.style.maxHeight = `${maxHeight}px`;
      const height = overlay.getBoundingClientRect().height;
      const below = view.innerHeight - safe - rect.bottom;
      const above = rect.top - safe;
      const preferred = height > below && above > below ? rect.top - 5 * scale - height : rect.bottom + 5 * scale;
      const left = Math.max(safe, Math.min(rect.left, view.innerWidth - safe - width)) / scale;
      const top = Math.max(safe, Math.min(preferred, view.innerHeight - safe - height)) / scale;
      setPosition(current => current?.left === left && current.top === top && current.width === width / scale && current.maxHeight === maxHeight && current.zoom === scale ? current : { position: 'fixed', left, top, width: width / scale, maxHeight, zoom: scale, zIndex: 30, '--general-select-left': `${left}px`, '--general-select-top': `${top}px`, '--general-select-max-height': `${maxHeight}px` } as CSSProperties);
    };
    update();
    const resize = new ResizeObserver(update);
    resize.observe(trigger); resize.observe(overlay);
    view.addEventListener('resize', update);
    view.addEventListener('scroll', update, true);
    view.visualViewport?.addEventListener('resize', update);
    return () => { resize.disconnect(); view.removeEventListener('resize', update); view.removeEventListener('scroll', update, true); view.visualViewport?.removeEventListener('resize', update); };
  }, [enabled, field, open, container, scale]);
  return { panel, position: enabled ? position : undefined };
}

function SelectList({ items, label, general }: { items: readonly SelectOption[]; label: string; general: boolean }) {
  return <ListBox className={`${styles.selectList}${general ? ` ${styles.generalSelectList}` : ''}`} aria-label={label} items={items} renderEmptyState={() => <div className={styles.selectEmpty} role="status">暂无可选项</div>}>
    {item => <ListBoxItem id={optionKey(item.value)} textValue={item.label} isDisabled={item.disabled} className={styles.selectOption} data-select-value={item.value} render={props => {
      // This finite, non-link item renders a div. Physical pointer motion shares
      // the same DOM focus path as keyboard navigation; it never commits a value.
      const divProps = props as ComponentProps<'div'>;
      return <div {...divProps} onPointerMove={event => {
        divProps.onPointerMove?.(event);
        if (!item.disabled && event.pointerType !== 'touch') event.currentTarget.focus({ preventScroll: true });
      }}/>;
    }}>
      {({ isSelected }) => <><span className={styles.selectCheck} aria-hidden="true">{isSelected ? <Icon name="check" size={12}/> : null}</span><span>{item.label}</span></>}
    </ListBoxItem>}
  </ListBox>;
}

/** Shared finite selection. Example: <Select label="外观" value={theme} onChange={setTheme} options={themes}/> */
export function Select({ label, value, onChange, children, options, disabled = false, state = 'ready', placeholder = '请选择', error, onRetry, hideLabel = false, appearance = 'default', id, 'aria-describedby': describedBy }: SelectProps) {
  const portal = useSelectPortal();
  const general = appearance === 'general-settings';
  const geometry = useGeneralSelectPosition(general, portal.field, portal.isOpen, portal.container, portal.scale);
  const items = options ?? optionsFromChildren(children);
  const selected = items.find(item => item.value === value);
  return <div ref={portal.field} className={styles.selectField} data-select-field>
    <AriaSelect className={styles.field} isOpen={portal.isOpen} onOpenChange={portal.onOpenChange} aria-label={hideLabel ? label : undefined} selectedKey={selected ? optionKey(value) : null} onSelectionChange={key => {
      const item = items.find(candidate => optionKey(candidate.value) === key);
      if (item && !item.disabled && state === 'ready') onChange(item.value);
    }} isDisabled={disabled || state === 'loading' || state === 'error'} isInvalid={state === 'error'} placeholder={placeholder}>
      {!hideLabel ? <Label data-field-label>{label}</Label> : null}
      <Button id={id} className={styles.selectTrigger} render={props => <button {...props} role="combobox" aria-label={label} aria-labelledby={undefined} aria-invalid={state === 'error' || undefined} title={selected?.label ?? placeholder}/>} aria-label={label} aria-describedby={describedBy} aria-invalid={state === 'error' || undefined} data-select-state={state} data-select-value={value}>
        <SelectValue className={styles.selectValue}>{({ selectedText }) => selectedText || placeholder}</SelectValue><span className={styles.selectChevron} aria-hidden="true"><Icon name="chevron-down" size={12}/></span>
      </Button>
      <Popover ref={geometry.panel} style={geometry.position} data-select-panel className={`${styles.selectPopover}${general ? ` ${styles.generalSelectPopover}` : ''}`} UNSTABLE_portalContainer={portal.container} placement="bottom start" offset={5 * portal.scale} containerPadding={12 * portal.scale}>
        <SelectList items={items} label={label} general={general}/>
      </Popover>
    </AriaSelect>
    {state === 'error' ? <div className={styles.selectError}><small role="alert">{error ?? '无法获取选项，已保存的配置仍保留。'}</small>{onRetry ? <Button className={styles.button} onPress={onRetry}>重试</Button> : null}</div> : null}
    {state === 'loading' ? <small className={styles.selectStatus} role="status">正在获取选项…</small> : null}
  </div>;
}
