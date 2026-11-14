import { useId, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { Button as AriaButton, Dialog, Menu, MenuItem, Popover, Separator } from 'react-aria-components';
import { Icon } from '../icons/Icon';
import styles from './overlays.module.css';

export type ActionMenuItem = { id: string; label: string; action: () => void; disabled?: boolean; checked?: boolean; separatorBefore?: boolean; external?: boolean; shortcut?: string; destructive?: boolean };
export function ActionMenu({ label, items }: { label: string; items: ActionMenuItem[] }) {
  const [open, setOpen] = useState(false);
  const [focusStrategy, setFocusStrategy] = useState<'first' | 'last' | undefined>(undefined);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const close = () => setOpen(false);
  return <>
    <AriaButton ref={triggerRef} className={styles.trigger} aria-label={label} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? menuId : undefined} data-tauri-drag-region="false"
      onPress={event => { setFocusStrategy(event.pointerType === 'keyboard' ? 'first' : undefined); setOpen(value => !value); }}
      onKeyDown={event => {
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault();
          setFocusStrategy(event.key === 'ArrowUp' ? 'last' : 'first');
          setOpen(true);
        }
      }}><Icon name="ellipsis"/></AriaButton>
    <Popover aria-label={label} triggerRef={triggerRef} scrollRef={menuRef} isOpen={open} onOpenChange={setOpen} className={styles.menuPopover} placement="top end" offset={6} containerPadding={8}>
      <Menu ref={menuRef} id={menuId} className={styles.menu} aria-label={label} autoFocus={focusStrategy} onClose={close}>
        {items.flatMap(item => [item.separatorBefore ? <Separator key={`${item.id}-separator`} className={styles.menuDivider}/> : null, <MenuItem className={styles.menuItem} key={item.id} id={item.id} data-destructive={item.destructive || undefined} onAction={() => { close(); item.action(); }} isDisabled={item.disabled} aria-description={item.checked === undefined ? undefined : item.checked ? '已开启' : '已关闭'}><span className={styles.menuOption}>{item.checked !== undefined ? <span className={styles.menuCheck} aria-hidden="true">{item.checked ? <Icon name="check" size={13}/> : null}</span> : null}{item.label}</span>{item.external ? <Icon name="external-link" size={13}/> : item.shortcut ? <span>{item.shortcut}</span> : null}</MenuItem>])}
      </Menu>
    </Popover>
  </>;
}
export function AnchoredPopover({ anchor, label, children, onClose }: { anchor: HTMLButtonElement; label: string; children: ReactNode; onClose: () => void }) {
  const triggerRef = useRef<HTMLButtonElement>(anchor);
  return <Popover className={styles.wordPopover} triggerRef={triggerRef} isOpen onOpenChange={open => { if (!open) onClose(); }} placement="top" offset={10}><Dialog className={styles.wordDialog} aria-label={label}>{children}</Dialog></Popover>;
}
