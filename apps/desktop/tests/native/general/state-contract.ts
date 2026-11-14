/** Fixed visual journeys through production controls, never caller-provided scripts. */
import type { GeneralVisualContext } from './native-context';
import { GeneralVisualPort } from './general-port';
import fixture from './fixture-data.json';

export const GENERAL_SWITCH_LABELS = ['可点击日语分词', '显示日语注音', '显示词性标记', '登录后打开 Luna'] as const;
export type VisualAction = Readonly<{ action: string; target: string; value?: string | boolean }>;
const pause = () => new Promise<void>(resolve => setTimeout(resolve, 16));
export async function waitForVisual(predicate: () => unknown, description: string, timeout = 9000): Promise<void> {
  const end = performance.now() + timeout;
  while (!predicate()) { if (performance.now() >= end) throw new Error('Visual journey timed out: ' + description); await pause(); }
}
const cleanText = (value: string | null | undefined) => (value ?? '').replace(/\s+/g, ' ').trim();
export function generalRoot(): HTMLElement {
  const element = document.querySelector<HTMLElement>('[data-ui-page="general-settings"]');
  if (!element) throw new Error('Real General production root is missing');
  return element;
}
function rootButton(name: string): HTMLButtonElement {
  const found = [...generalRoot().querySelectorAll<HTMLButtonElement>('button')].filter(node => cleanText(node.textContent) === name);
  if (found.length !== 1) throw new Error('Unique production button required: ' + name);
  return found[0];
}
function switchInput(label: string): HTMLInputElement {
  const input = generalRoot().querySelector<HTMLInputElement>(`input[role="switch"][aria-label="${label}"]`);
  if (!input) throw new Error('Production switch missing: ' + label);
  return input;
}
const disabled = (node: Element) => node.matches(':disabled') || node.getAttribute('aria-disabled') === 'true';
function scopeTrigger(): HTMLButtonElement {
  const trigger = generalRoot().querySelector<HTMLButtonElement>('[role="combobox"][aria-label="阅读偏好作用范围"]');
  if (!trigger) throw new Error('Production scope selector missing');
  return trigger;
}
export async function prepareGeneralVisualCase(context: GeneralVisualContext, port: GeneralVisualPort): Promise<VisualAction[]> {
  const actions: VisualAction[] = [];
  const toggle = async (label: typeof GENERAL_SWITCH_LABELS[number], selected: boolean) => {
    const input = switchInput(label);
    if (input.checked === selected) return;
    if (disabled(input)) throw new Error('The visual journey attempted a disabled control: ' + label);
    // Direct checkbox activation avoids a synthetic label default-action plus
    // React Aria virtual press toggling twice. The real input/change path runs.
    input.click(); actions.push({ action: 'click_switch_input', target: label, value: selected });
    await waitForVisual(() => switchInput(label).checked === selected, 'switch ' + label);
  };
  const selectGame = async () => {
    const trigger = scopeTrigger(); if (disabled(trigger)) throw new Error('Scope is not editable');
    trigger.click(); actions.push({ action: 'open_scope', target: '阅读偏好作用范围' });
    const name = `仅此游戏 · ${port.state.games[0]?.displayName}`;
    await waitForVisual(() => [...document.querySelectorAll('[role="option"]')].some(node => cleanText(node.textContent) === name), 'game scope option');
    const option = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find(node => cleanText(node.textContent) === name)!;
    option.click(); actions.push({ action: 'choose_scope', target: name, value: `game:${fixture.gameId}` });
    await pause();
  };
  await waitForVisual(() => document.querySelector('[data-ui-page="general-settings"]'), 'real General route');
  if (context.stateId.endsWith('-loading') && context.subcase === 'initial') {
    await waitForVisual(() => port.scopedPreferencePending.has('general-visual-initial-read'), 'initial controlled read');
  } else {
    await waitForVisual(() => generalRoot().querySelector('fieldset') && !generalRoot().querySelector('[aria-busy="true"]'), 'initial General scope');
    if (context.stateId.endsWith('-loading')) {
      port.scopedPreferenceResponses.push({ scopeKey: `game:${fixture.gameId}`, holdKey: 'general-visual-refresh-read' });
      await selectGame();
      await waitForVisual(() => port.scopedPreferencePending.has('general-visual-refresh-read'), 'held next-scope read with last-known values');
    } else if (context.stateId.endsWith('-error')) {
      // Exercise the real stale-draft guard: a completed save is followed by an
      // authority reread failure while later edits remain in the same editor.
      await toggle('显示日语注音', false);
      port.preferenceSaveHoldKeys.push('general-visual-read-error-save');
      rootButton('保存阅读偏好').click(); actions.push({ action: 'save_held', target: '保存阅读偏好' });
      await waitForVisual(() => port.preferenceSavePending.has('general-visual-read-error-save'), 'held save before reread failure');
      await toggle('显示日语注音', true); await toggle('显示词性标记', true);
      port.scopedPreferenceResponses.push({ scopeKey: 'global', error: fixture.readError });
      port.releasePreferenceSave('general-visual-read-error-save'); actions.push({ action: 'release_save_then_fail_read', target: 'global' });
      await waitForVisual(() => [...generalRoot().querySelectorAll('[role="alert"]')].some(node => node.textContent?.includes(fixture.readError)), 'read error displayed');
    } else if (context.stateId.endsWith('-disabled')) {
      await toggle('显示词性标记', true); await toggle('可点击日语分词', false);
    } else if (context.stateId.endsWith('-scope-game') || context.stateId.endsWith('-long-text')) {
      await selectGame();
      await waitForVisual(() => scopeTrigger().dataset.selectValue === `game:${fixture.gameId}` && !generalRoot().querySelector('[aria-busy="true"]'), 'game scope loaded');
    } else if (context.stateId.endsWith('-restore-inheritance')) {
      await selectGame();
      await waitForVisual(() => scopeTrigger().dataset.selectValue === `game:${fixture.gameId}` && switchInput('显示日语注音').checked === false, 'saved game override');
      rootButton('恢复继承全局').click(); actions.push({ action: 'restore_inheritance_draft_only', target: `game:${fixture.gameId}` });
      await waitForVisual(() => switchInput('显示日语注音').checked && !disabled(rootButton('保存阅读偏好')), 'inherited draft without save');
    } else if (context.stateId.endsWith('-save-failed')) {
      await toggle('显示词性标记', true);
      port.scopedPreferenceSaveResponses.push({ error: fixture.saveError });
      rootButton('保存阅读偏好').click(); actions.push({ action: 'save_failed', target: '保存阅读偏好' });
      await waitForVisual(() => [...generalRoot().querySelectorAll('[role="alert"]')].some(node => node.textContent?.includes(fixture.saveError)), 'save failure displayed');
    } else if (context.stateId.endsWith('-keyboard-focus-escape')) {
      const trigger = scopeTrigger(); trigger.focus(); trigger.click(); actions.push({ action: 'open_scope_keyboard_state', target: '阅读偏好作用范围' });
      await waitForVisual(() => Boolean(document.querySelector('[role="listbox"]')), 'scope listbox');
      const send = (key: string) => { (document.activeElement ?? trigger).dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })); actions.push({ action: 'key', target: 'scope-menu', value: key }); };
      send('Home'); await pause(); send('End'); await pause();
      if (context.subcase !== 'menu-open') { send('Escape'); await waitForVisual(() => !document.querySelector('[role="listbox"]'), 'Escape closes scope menu'); await waitForVisual(() => document.activeElement === trigger, 'focus returns to real scope trigger'); }
    }
  }
  if (!context.stateId.endsWith('-keyboard-focus-escape')) {
    // Let the real Select overlay's scheduled focus restoration finish before
    // the predeclared normal-content blur. Never remove focus CSS or change a
    // key-state assertion to hide a late lifecycle operation.
    await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    actions.push({ action: 'settle_pending_focus_restore', target: 'document' });
    (document.activeElement as HTMLElement | null)?.blur(); actions.push({ action: 'blur_active_control', target: 'document' });
  }
  const scroller = generalRoot().querySelector<HTMLElement>('[data-ui-scroll="preferences"]');
  if (!scroller) throw new Error('Real General scroll owner missing');
  scroller.scrollTop = context.subcase === 'content-end' ? scroller.scrollHeight : 0;
  actions.push({ action: 'scroll_owner', target: 'preferences', value: context.subcase === 'content-end' ? 'end' : 'top' });
  await document.fonts.ready;
  await new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
  return actions;
}
function rectangle(node: Element | null) {
  if (!node) return null;
  const r = node.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height };
}
function font(node: Element | null) {
  if (!node) return null; const s = getComputedStyle(node);
  return { family: s.fontFamily, size: s.fontSize, weight: s.fontWeight, lineHeight: s.lineHeight };
}
export function collectGeneralVisualFacts(context: GeneralVisualContext, port: GeneralVisualPort, actions: readonly VisualAction[]): Record<string, unknown> {
  const root = generalRoot(), trigger = scopeTrigger();
  const sections = [...root.querySelectorAll<HTMLElement>('section')];
  const sectionCard = (name: string) => sections.find(section => cleanText(section.querySelector('h2')?.textContent) === name)?.querySelector('h2+div') ?? null;
  const sidebar = document.querySelector('aside'), nav = document.querySelector('nav[aria-label="设置分类"]');
  const visible = (node: Element) => Boolean(node.getClientRects().length) && getComputedStyle(node).visibility !== 'hidden';
  const switches = [...root.querySelectorAll<HTMLInputElement>('input[role="switch"]')].map(node => ({ label: node.getAttribute('aria-label'), selected: node.checked, disabled: disabled(node), rect: rectangle(node.closest('label')) }));
  const buttons = [...document.querySelectorAll<HTMLButtonElement>('button')].filter(visible).map(node => ({ label: cleanText(node.textContent), selected: node.getAttribute('aria-current') === 'page', disabled: disabled(node), rect: rectangle(node) }));
  const footer = root.querySelector('footer'), save = rootButton('保存阅读偏好'), discard = rootButton('放弃修改');
  const focused = document.activeElement as HTMLElement | null;
  const geometry = { frame: rectangle(document.querySelector('#root>div')), sidebar: rectangle(sidebar), sidebar_footer: rectangle(sidebar?.lastElementChild ?? null),
    header: rectangle(root.querySelector('h1')?.closest('header') ?? root.querySelector('h1')?.parentElement?.parentElement ?? null), title: rectangle(root.querySelector('h1')),
    scroll: rectangle(root.querySelector('[data-ui-scroll="preferences"]')), lexical_card: rectangle(sectionCard('分词与词典')), records_card: rectangle(sectionCard('阅读记录')),
    local_card: rectangle(sectionCard('本机偏好')), footer: rectangle(footer), scope: rectangle(trigger),
    fonts: { heading: font(root.querySelector('h1')), sidebar: font(sidebar), footer: font(footer), navigation: font(nav?.querySelector('button') ?? null) } };
  const controls = Object.fromEntries(switches.map(row => [row.label ?? '', { selected: row.selected, disabled: row.disabled }]));
  const menuOpen = trigger.getAttribute('aria-expanded') === 'true';
  const alerts = [...root.querySelectorAll('[role="alert"]')].map(node => cleanText(node.textContent));
  const busy = root.querySelector('[aria-busy]')?.getAttribute('aria-busy') === 'true';
  const caseAssertions = [
    { id: 'real_general_root', actual: root.dataset.uiPage, expected: 'general-settings' },
    { id: 'theme', actual: document.documentElement.dataset.theme, expected: context.theme },
    { id: 'zoom', actual: Number.parseFloat(getComputedStyle(document.documentElement).zoom), expected: context.zoom },
    { id: 'no_forbidden_effects', actual: port.forbiddenEffects, expected: [] },
    { id: 'no_startup_writes', actual: port.loginStartupRequests, expected: [] },
    { id: 'lexical_card_present', actual: Boolean(sectionCard('分词与词典')), expected: !(context.stateId.endsWith('-loading') && context.subcase === 'initial') },
  ];
  if (context.stateId.endsWith('-loading')) caseAssertions.push({ id: 'reading_pending', actual: busy, expected: true });
  if (context.stateId.endsWith('-disabled')) caseAssertions.push({ id: 'pos_effectively_disabled', actual: controls['显示词性标记']?.disabled, expected: true });
  if (context.stateId.endsWith('-error') || context.stateId.endsWith('-save-failed')) caseAssertions.push({ id: 'error_visible', actual: alerts.length > 0, expected: true });
  if (context.stateId.endsWith('-keyboard-focus-escape')) caseAssertions.push({ id: 'scope_menu_state', actual: menuOpen, expected: context.subcase === 'menu-open' });
  const resources = performance.getEntriesByType('resource').map(item => item.name);
  if (resources.length > 128) throw new Error('The bounded verification entry loaded too many resources');
  return { title: document.title, headings: [...root.querySelectorAll('h1,h2')].map(node => cleanText(node.textContent)), text: document.body.innerText,
    switches, buttons, scope: { kind: trigger.dataset.selectValue === 'global' ? 'global' : 'game', label: cleanText(trigger.textContent), disabled: disabled(trigger), expanded: menuOpen, raw: trigger.dataset.selectValue ?? null },
    status: cleanText(footer?.textContent), dirty: !disabled(discard) || !disabled(save), menuOpen,
    focused: focused ? { tag: focused.tagName, label: focused.getAttribute('aria-label') ?? cleanText(focused.textContent).slice(0, 160), role: focused.getAttribute('role') } : null,
    geometry, scrolls: [...document.querySelectorAll<HTMLElement>('[data-ui-scroll="preferences"],aside')].map(node => ({ owner: node.matches('aside') ? 'sidebar' : 'preferences', top: node.scrollTop, height: node.clientHeight, scrollHeight: node.scrollHeight, scrollWidth: node.scrollWidth, width: node.clientWidth })),
    theme: document.documentElement.dataset.theme, zoom: Number.parseFloat(getComputedStyle(document.documentElement).zoom), fontStatus: document.fonts.status,
    documentVisibility: document.visibilityState, hasFocus: document.hasFocus(), userAgent: navigator.userAgent,
    replay: { stateId: context.stateId, subcase: context.subcase, fixture: structuredClone(fixture), feedback: alerts }, errors: [],
    renderSource: 'production-WindowRoot', provenance: { entry: 'tests/native/general/entry.tsx', controlledTransport: true, productionBootstrapImported: false,
      osRuntimeVerified: false, storageRuntimeVerified: false, gameExecutionVerified: false }, actions,
    controlledCommands: port.commands.map(item => item.command), readQueries: port.scopedPreferenceQueries,
    saveCalls: port.visualSaveCalls.map(item => item.command), startupWrites: port.loginStartupRequests,
    forbiddenEffects: port.forbiddenEffects, caseAssertions, loadedResources: resources };
}
