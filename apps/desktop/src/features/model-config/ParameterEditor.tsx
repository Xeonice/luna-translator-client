import { ConfigurationRow as FormRow } from './ConfigurationFields';
import type { Parameter } from '../../bridge/contracts.generated';
import { Select, TextField } from '../../ui/primitives/Controls';
import styles from './styles.module.css';

/** Preserve inherit / explicit set (including zero) / omit as distinct commands. */
export function ParameterEditor({ parameters, onChange }: { parameters: Record<string, Parameter | undefined>; onChange: (value: Record<string, Parameter>) => void }) {
  return <>{[['temperature', 'Temperature'], ['max_tokens', '最大输出 Token']].map(([key, label]) => {
    const parameter = parameters[key] ?? { mode: 'inherit' as const, value: null };
    const change = (patch: Partial<Parameter>) => { const next: Record<string, Parameter> = {}; for (const [name, value] of Object.entries(parameters)) if (value) next[name] = value; next[key] = { ...parameter, ...patch }; onChange(next); };
    return <FormRow label={label} key={key} note="设置为 0 会保留；省略时不发送该参数。"><div className={styles.parameter}><Select label={`${label} 参数策略`} value={parameter.mode} onChange={mode => change({ mode: mode as Parameter['mode'], value: mode === 'set' ? parameter.value ?? 0 : null })}><option value="inherit">继承</option><option value="set">显式设置</option><option value="omit">省略</option></Select>{parameter.mode === 'set' ? <TextField label={`${label} 值`} value={parameter.value === null || parameter.value === undefined ? '' : String(parameter.value)} onChange={value => { if (value.trim() === '') change({ value: null }); else { const numeric = Number(value); if (Number.isFinite(numeric)) change({ value: numeric }); } }} type="number"/> : null}</div></FormRow>;
  })}</>;
}
