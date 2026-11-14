/** General V3 observer migration only. No arbitrary source URL or rendering code. */
import fs from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync,spawn} from 'node:child_process';
const repository=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../../..');
const helper='scripts/quality/general_capture_provider.py';
export function generalSourceCaptureInputs(){
  return JSON.parse(execFileSync('python3',[path.join(repository,helper),'--inputs'],{cwd:repository,encoding:'utf8'}));
}
export async function captureGeneralSourceQualification({base=repository,output,width,height,theme,zoom,savedGame=false}){
  const canvas=`${width}x${height}-${theme}-z${zoom}`;
  if(!/^(1000x720|760x560)-(dark|light)-z[12]$/.test(canvas)||typeof savedGame!=='boolean')throw new Error('Finite General canvas required');
  const root=await fs.realpath(base),target=path.resolve(output);
  if(!target.startsWith(path.join(root,'.build')+path.sep))throw new Error('Qualification output must be private to its declared checkout');
  const command=['python3',path.join(root,helper),'--canvas',canvas,'--output',target,...(savedGame?['--saved-game']:[])];
  const actual=await new Promise((resolve,reject)=>{const child=spawn(command[0],command.slice(1),{cwd:root,stdio:['ignore','pipe','pipe']}),out=[],err=[];child.stdout.on('data',b=>out.push(b));child.stderr.on('data',b=>err.push(b));child.on('error',reject);child.on('close',code=>resolve({code,stdout:Buffer.concat(out).toString(),stderr:Buffer.concat(err).toString()}));});
  await fs.mkdir(path.dirname(target),{recursive:true});
  await fs.writeFile(target+'.invocation.json',JSON.stringify({command,...actual},null,2)+'\n',{flag:'wx'});
  if(actual.code!==0)throw new Error('Qualified WK source observer failed: '+actual.stdout+' '+actual.stderr);
  const reply=JSON.parse(actual.stdout),folder=path.join(root,reply.output);
  if(reply.status!=='passed'||reply.contract.threshold!==0||reply.contract.mask!==false||reply.contract.product_accepted!==false)throw new Error('General observer receipt scope differs');
  return {left:await fs.readFile(path.join(folder,'document-1-frame-1.png')),right:await fs.readFile(path.join(folder,'document-2-frame-1.png')),
          receipt:reply,rawResult:JSON.parse(await fs.readFile(path.join(root,reply.result),'utf8'))};
}
