import { useEffect } from 'react';
import type { WindowRole } from '../bridge/application-port';
import { ApplicationStore, useApplication } from '../bridge/projections';
import { ErrorBoundary } from './ErrorBoundary';
import { MainWindow } from '../windows/main/MainWindow';
import { HudWindow } from '../windows/hud/HudWindow';
import { SettingsWindow } from '../windows/settings/SettingsWindow';
import { LearningWindow } from '../windows/learning/LearningWindow';

export function WindowRoot({ role, store }: { role: WindowRole; store: ApplicationStore }) {
  const { snapshot } = useApplication(store);
  useEffect(() => {
    const preference = snapshot?.preferences;
    const media = matchMedia('(prefers-color-scheme: light)');
    const apply = () => { document.documentElement.dataset.theme = preference?.theme === 'light' || (preference?.theme === 'system' && media.matches) ? 'light' : 'dark'; document.documentElement.dataset.transparency = preference?.transparencyMode ?? 'material'; };
    apply(); media.addEventListener('change', apply); return () => media.removeEventListener('change', apply);
  }, [snapshot?.preferences.theme, snapshot?.preferences.transparencyMode]);
  return <ErrorBoundary>{role === 'hud' ? <HudWindow store={store}/> : role === 'settings' ? <SettingsWindow store={store}/> : role === 'learning' ? <LearningWindow store={store}/> : <MainWindow store={store}/>}</ErrorBoundary>;
}
