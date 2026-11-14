/** Build-only copy of the exact user-approved General reference closure. */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const sourcePrefix = 'docs/design/first-product-v1/candidates/general-state-evidence-20261108/';
const approvalPath = 'docs/development/design-approval-general-states-20261108.json';
const approvalHash = '6e1694583d1c12a7597c9b00cf91dd21154a04f6f463fc4a9aa48a5042a6cb8a';
const promotionHash = '345205c37d1b77df870b429efbd84c6811c34f1416bbd0b1cc8cac1e81547b84';
export const GENERAL_REFERENCE_FILES = Object.freeze(['settings.html','state-gap.css','state-gap.js','assets/icons.js','assets/material-tokens.css','source/settings.css','source/settings.js']);
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function exactFile(relative, root = repository) {
  if (typeof relative !== 'string' || !relative || relative.startsWith('/') || relative.split('/').some(part => !part || part==='.' || part==='..') || /[\\:#?\x00-\x1f]/.test(relative)) throw new Error('Safe General asset source required');
  let current=root;
  for (const part of relative.split('/')) { current=path.join(current,part); if (fs.lstatSync(current).isSymbolicLink()) throw new Error('General source symlink forbidden'); }
  return fs.readFileSync(current);
}
export function readGeneralVisualAssets(root = repository) {
  const approvalBytes=exactFile(approvalPath,root);
  if (hash(approvalBytes)!==approvalHash) throw new Error('General state source approval identity differs');
  const approval=JSON.parse(approvalBytes);
  if (approval.status!=='approved_by_user' || approval.native_verified!==false || approval.product_accepted!==false) throw new Error('General source approval scope differs');
  const promotionBytes=exactFile(sourcePrefix+'promotion.json',root);
  if (hash(promotionBytes)!==promotionHash) throw new Error('General source promotion identity differs');
  const promotion=JSON.parse(promotionBytes);
  const input={};
  for (const name of GENERAL_REFERENCE_FILES) {
    const relative=sourcePrefix+name,record=promotion.relocation.find(row=>row.promoted.path===relative);
    if (!record || record.transformation!=='verbatim' || record.promoted.sha256!==record.captured.sha256 || record.promoted.bytes!==record.captured.bytes) throw new Error('Unapproved General source transform');
    const bytes=exactFile(relative,root);
    if (hash(bytes)!==record.promoted.sha256 || bytes.length!==record.promoted.bytes) throw new Error('General source asset changed: '+name);
    input[name]={bytes,sha256:hash(bytes),source:relative};
  }
  return input;
}
export function generalVisualAssets() {
  let verified;
  return {name:'general-user-approved-reference-closure',apply:'build',
    buildStart(){verified=readGeneralVisualAssets();},
    generateBundle(){
      const current=readGeneralVisualAssets();
      for (const name of GENERAL_REFERENCE_FILES) {
        if (verified[name].sha256!==current[name].sha256) throw new Error('General source changed while bundling');
        this.emitFile({type:'asset',fileName:'general-reference/'+name,source:current[name].bytes});
      }
      this.emitFile({type:'asset',fileName:'general-reference/manifest.json',source:JSON.stringify({schema_version:1,kind:'general-native-reference-assets/v1',approval:{path:approvalPath,sha256:approvalHash},promotion:{path:sourcePrefix+'promotion.json',sha256:promotionHash},files:Object.fromEntries(GENERAL_REFERENCE_FILES.map(name=>[name,{source:current[name].source,sha256:current[name].sha256,bytes:current[name].bytes.length}])),native_context:'user-approved-transparent-body-and-system-chrome',product_accepted:false},null,2)+'\n'});
    }};
}

export function generalVisualBuildInputs() {
  return {main:path.join(repository,'apps/desktop/index.html'),'general-visual':path.join(repository,'apps/desktop/general-visual.html'),'general-reference-replay':path.join(repository,'apps/desktop/tests/native/general/reference-replay.js')};
}
