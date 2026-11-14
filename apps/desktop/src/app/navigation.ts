import type { WindowRole } from '../bridge/contracts.generated';
import type { IconName } from '../ui/icons/registry';
export type MainPage = 'library' | 'session' | 'history' | 'manual' | 'diagnostics';
export type SettingsPage = 'general' | 'capture' | 'ocr' | 'translation' | 'processing' | 'speech' | 'display' | 'network' | 'data' | 'diagnostics';
export const mainNavigation: { id: MainPage; label: string; icon: IconName; group: string }[] = [
  { id: 'library', label: '游戏库', icon: 'grid', group: '游戏' },
  { id: 'session', label: '当前会话', icon: 'activity', group: '游戏' },
  { id: 'history', label: '历史记录', icon: 'history', group: '文本' },
  { id: 'manual', label: '手动翻译', icon: 'languages', group: '文本' },
  { id: 'diagnostics', label: '诊断', icon: 'activity', group: '工具' },
];

/** Shared by production and controlled UI bootstrap; native producer uses this query contract. */
export function windowRoleFromUrl(url: string): WindowRole {
  const requested = new URL(url, 'https://app.localhost/').searchParams.get('role');
  return requested === 'hud' || requested === 'settings' || requested === 'learning' ? requested : 'main';
}
