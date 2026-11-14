import { useEffect, useState } from 'react';
import type { CatalogPurpose, OfflineEngineDescriptor, ProviderPresetDescriptor, ServiceCatalog, ServiceFormTarget } from '../../bridge/contracts.generated';
import { Button, Select, Sheet, TextField } from '../../ui/primitives/Controls';
import styles from './guided.module.css';

export interface ServiceCatalogSelection { target: ServiceFormTarget; purpose: CatalogPurpose }
export interface ServiceCatalogPickerProps {
  catalog: ServiceCatalog;
  open: boolean;
  initialPurpose?: CatalogPurpose;
  busy?: boolean;
  onClose: () => void;
  onSelect: (selection: ServiceCatalogSelection) => void;
}
export const guidedPurposeNames: Record<CatalogPurpose, string> = { translation: '翻译', grammar: '句子精读', context_word: '语境查词' };
type Entry = { descriptor: ProviderPresetDescriptor | OfflineEngineDescriptor; target: ServiceFormTarget; label: string; description: string; key: string };

function entries(catalog: ServiceCatalog): Entry[] {
  return [
    ...catalog.presets.map(descriptor => ({ descriptor, target: { kind: 'http' as const, preset: descriptor.key }, label: descriptor.interfaceLabel, description: descriptor.interfaceDescription, key: `http:${descriptor.key.presetId}@${descriptor.key.presetRevision}` })),
    ...catalog.offlineEngines.map(descriptor => ({ descriptor, target: { kind: 'offline' as const, engineId: descriptor.engineId, contractRevision: descriptor.contractRevision }, label: descriptor.label, description: '使用本机已有的翻译引擎；安装和语言条件在配置时检查。', key: `offline:${descriptor.engineId}@${descriptor.contractRevision}` })),
  ];
}

/** Catalog describes what can be configured; choosing a row never activates a service. */
export function ServiceCatalogPicker({ catalog, open, initialPurpose = 'translation', busy = false, onClose, onSelect }: ServiceCatalogPickerProps) {
  const [purpose, setPurpose] = useState<CatalogPurpose>(initialPurpose);
  const [search, setSearch] = useState('');
  const [brand, setBrand] = useState<string | null>(null);
  useEffect(() => { if (open) { setPurpose(initialPurpose); setSearch(''); setBrand(null); } }, [open, initialPurpose]);
  const all = entries(catalog);
  const selectedBrand = catalog.brands.find(item => item.brandId === brand);
  const options = all.filter(item => item.descriptor.brandId === brand && item.descriptor.allowedPurposes.includes(purpose));
  const searchKey = search.trim().toLocaleLowerCase();
  const brands = catalog.brands.filter(item => !searchKey || [item.label, ...all.filter(entry => entry.descriptor.brandId === item.brandId).map(entry => `${entry.descriptor.label} ${entry.label}`)].join(' ').toLocaleLowerCase().includes(searchKey));
  const choose = (entry: Entry) => { if (!busy) onSelect({ target: entry.target, purpose }); };
  return <Sheet title={selectedBrand ? `选择${selectedBrand.label}的连接方式` : '添加服务'} description={selectedBrand ? '只按你选定的接口配置；发生错误时不会自动换成其他接口。' : '选择你已经使用的服务，或本机已有的翻译引擎。'} open={open} onClose={onClose} footer={<><Button disabled={busy} onPress={selectedBrand ? () => setBrand(null) : onClose}>{selectedBrand ? '返回服务列表' : '取消'}</Button></>}>
    <div className={styles.catalog} data-service-catalog>
      {selectedBrand ? <div className={styles.interfaces} aria-label={`${selectedBrand.label}连接方式`}>
        {options.map(entry => <Button key={entry.key} disabled={busy} onPress={() => choose(entry)}><span className={styles.interfaceText}><strong>{entry.label}</strong><small>{entry.description}</small></span></Button>)}
      </div> : <>
        <div className={styles.catalogPurpose}><Select label="服务用途" value={purpose} disabled={busy} onChange={value => { setPurpose(value as CatalogPurpose); setBrand(null); }}>{Object.entries(guidedPurposeNames).map(([value, label]) => <option value={value} key={value}>{label}</option>)}</Select></div>
        <TextField label="搜索服务" placeholder="搜索服务名称" value={search} onChange={setSearch} disabled={busy}/>
        <div className={styles.brands} aria-label="服务品牌">
          {brands.map(item => {
            const matching = all.filter(entry => entry.descriptor.brandId === item.brandId && entry.descriptor.allowedPurposes.includes(purpose));
            return <Button key={item.brandId} disabled={busy || matching.length === 0} onPress={() => matching.length === 1 ? choose(matching[0]) : setBrand(item.brandId)}><span className={styles.brandText}><strong>{item.label}</strong><small>{matching.length === 0 ? '不提供此用途' : matching.every(entry => entry.target.kind === 'offline') ? '本机翻译引擎' : matching.length > 1 ? '选择明确的连接方式' : matching[0].description}</small></span></Button>;
          })}
        </div>
        {brands.length === 0 ? <p className={styles.note} role="status">没有匹配的服务。</p> : null}
      </>}
    </div>
  </Sheet>;
}
