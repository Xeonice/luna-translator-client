#!/usr/bin/env node
/** Read-only TypeScript AST inventory. No product execution or approval. */
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const args=process.argv.slice(2),ri=args.indexOf('--root');
const root=path.resolve(ri<0?process.cwd():args[ri+1]);
let ts;
const starts=[root,path.dirname(fileURLToPath(import.meta.url))];
for(const start of starts){for(let parent=start;;parent=path.dirname(parent)){
  try{ts=createRequire(path.join(parent,'apps/desktop/package.json'))('typescript');break;}catch{}
  if(parent===path.dirname(parent))break;
}if(ts)break;}
if(!ts)throw Error('Existing TypeScript installation required; do not install or substitute a regex parser.');
const relative=p=>path.relative(root,p).split(path.sep).join('/');
function checked(p){const parts=p.split('/');let current=root;if(fs.lstatSync(root).isSymbolicLink())throw Error('symlink root');for(const part of parts){if(!part||part==='.'||part==='..')throw Error('unsafe source path');current=path.join(current,part);if(fs.lstatSync(current).isSymbolicLink())throw Error('symlink source/ancestor: '+p);}return current;}
const read=p=>fs.readFileSync(checked(p),'utf8');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
// A single Program supplies declaration identity across aliases, namespaces and barrels.
// No diagnostics are used as a substitute for the dedicated product typecheck.
const sourceFiles=[];
function listSources(dir){for(const item of fs.readdirSync(checked(dir),{withFileTypes:true})){const file=dir+'/'+item.name;if(item.isDirectory())listSources(file);else if(/\.[jt]sx?$/.test(item.name))sourceFiles.push(file);}}
listSources('apps/desktop/src');
const program=ts.createProgram(sourceFiles.map(checked),{jsx:ts.JsxEmit.Preserve,target:ts.ScriptTarget.Latest,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,allowJs:true,skipLibCheck:true,noEmit:true});
const checker=program.getTypeChecker(),cache=new Map(),errors=[];
function source(p){if(!cache.has(p))cache.set(p,program.getSourceFile(checked(p))??ts.createSourceFile(p,read(p),ts.ScriptTarget.Latest,true,p.endsWith('x')?ts.ScriptKind.TSX:ts.ScriptKind.TS));return cache.get(p);}
function unwrap(node){while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;}
function symbolAt(node){if(!node)return undefined;let symbol=checker.getSymbolAtLocation(node);if(!symbol&&ts.isPropertyAccessExpression(node))symbol=checker.getSymbolAtLocation(node.name);const seen=new Set();while(symbol&&(symbol.flags&ts.SymbolFlags.Alias)&&!seen.has(symbol)){seen.add(symbol);symbol=checker.getAliasedSymbol(symbol);}return symbol;}
function valueDeclaration(node){const symbol=symbolAt(node);return symbol?.valueDeclaration??symbol?.declarations?.find(d=>!ts.isImportSpecifier(d)&&!ts.isExportSpecifier(d));}
function initializer(node){const declaration=valueDeclaration(node);return declaration&&ts.isVariableDeclaration(declaration)?declaration.initializer:undefined;}
function identity(symbol){const declaration=symbol?.valueDeclaration??symbol?.declarations?.[0];return declaration?relative(declaration.getSourceFile().fileName)+':'+declaration.pos+':'+symbol.getName():null;}
function exportedSymbols(file){const symbol=checker.getSymbolAtLocation(source(file));return symbol?checker.getExportsOfModule(symbol):[];}
function resolvedExport(file,name){let symbol=exportedSymbols(file).find(s=>s.getName()===name);if(symbol?.flags&ts.SymbolFlags.Alias)symbol=checker.getAliasedSymbol(symbol);return identity(symbol);}
function unknown(kind,file,node,detail){const sf=node.getSourceFile(),position=sf.getLineAndCharacterOfPosition(node.getStart());errors.push('Unknown '+kind+': '+relative(sf.fileName)+':'+(position.line+1)+': '+detail);}
function reactImport(file,tag){
  const head=tag.getText().split('.')[0];
  return source(file).statements.some(statement=>{
    if(!ts.isImportDeclaration(statement)||statement.moduleSpecifier.text!=='react'||!statement.importClause)return false;
    const clause=statement.importClause;return clause.name?.text===head||clause.namedBindings&&(ts.isNamespaceImport(clause.namedBindings)?clause.namedBindings.name.text===head:clause.namedBindings.elements.some(item=>item.name.text===head));
  });
}
function componentSymbol(node,seen=new Set()){
  const symbol=symbolAt(node);if(!symbol||seen.has(symbol))return symbol;seen.add(symbol);
  const declaration=symbol.valueDeclaration;
  if(declaration&&ts.isVariableDeclaration(declaration)){const value=unwrap(declaration.initializer);if(value&&(ts.isIdentifier(value)||ts.isPropertyAccessExpression(value)))return componentSymbol(value,seen);}
  return symbol;
}
function componentDeclaration(node){const symbol=componentSymbol(node);return symbol?.valueDeclaration??symbol?.declarations?.[0];}
function literalString(node,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node))return undefined;seen.add(node);
  if(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node))return node.text;
  const value=ts.isIdentifier(node)?initializer(node):undefined;return value?literalString(value,seen):undefined;
}
function contains(node,predicate){let found=false;walk(node,n=>{if(predicate(n))found=true;});return found;}
function dependsOn(node,target,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node))return false;seen.add(node);
  if(ts.isIdentifier(node)&&symbolAt(node)===target)return true;
  const initial=ts.isIdentifier(node)?initializer(node):undefined;
  if(initial&&dependsOn(initial,target,seen))return true;
  return ts.forEachChild(node,child=>dependsOn(child,target,seen))===true;
}

function walk(n,fn){fn(n);ts.forEachChild(n,c=>walk(c,fn));}
function strings(n){const values=[];walk(n,c=>{if(ts.isStringLiteral(c))values.push(c.text);});return values;}
function declarations(p,name){let result;walk(source(p),n=>{if(ts.isVariableDeclaration(n)&&n.name.getText()===name)result=n.initializer;});return result;}
function union(p,name){let values=[];walk(source(p),n=>{if(ts.isTypeAliasDeclaration(n)&&n.name.text===name)values=strings(n.type);});return values.sort();}
function objects(p,name,key){const n=declarations(p,name),values=[];if(!n)return values;walk(n,c=>{if(ts.isObjectLiteralExpression(c)){const prop=c.properties.find(v=>ts.isPropertyAssignment(v)&&v.name.getText()===key);if(prop&&ts.isStringLiteral(prop.initializer))values.push(prop.initializer.text);}});return values.sort();}
function resolveImport(from,spec){if(!spec.startsWith('.'))return null;const base=path.resolve(root,path.dirname(from),spec);for(const candidate of [base,...['.ts','.tsx','.js','.jsx','.json','.css'].map(ext=>base+ext),...['index.ts','index.tsx','index.js'].map(n=>path.join(base,n))])if(fs.existsSync(candidate)&&fs.statSync(candidate).isFile())return relative(candidate);return null;}
function imports(p){if(!/\.[jt]sx?$/.test(p))return [];const values=[];walk(source(p),n=>{if((ts.isImportDeclaration(n)||ts.isExportDeclaration(n))&&n.moduleSpecifier&&ts.isStringLiteral(n.moduleSpecifier)){const resolved=resolveImport(p,n.moduleSpecifier.text);if(resolved)values.push(resolved);}});return [...new Set(values)].sort();}
function closure(roots){const seen=new Set();function visit(p){if(seen.has(p))return;seen.add(p);for(const dep of imports(p))visit(dep);if(p.endsWith('.css')){const css=read(p);for(const m of css.matchAll(/@import\s+['"]([^'"]+)['"]/g)){const dep=resolveImport(p,m[1]);if(dep)visit(dep);}}}roots.forEach(visit);return [...seen].sort();}
const nav='apps/desktop/src/app/navigation.ts',windowRoot='apps/desktop/src/app/WindowRoot.tsx',bootstrap='apps/desktop/src/app/bootstrap.tsx',settings='apps/desktop/src/windows/settings/SettingsWindow.tsx';
const mainPages=union(nav,'MainPage'),settingsPages=union(nav,'SettingsPage'),mainNav=objects(nav,'mainNavigation','id'),settingsNav=objects(settings,'destinations','id');
const windowRoles=new Set(),surfaceValues=new Set();
const roleSymbols=new Set();
walk(source(windowRoot),node=>{
  if(!ts.isFunctionDeclaration(node)||node.name?.text!=='WindowRoot')return;
  for(const parameter of node.parameters)if(ts.isObjectBindingPattern(parameter.name))for(const element of parameter.name.elements)if((element.propertyName?.getText()??element.name.getText())==='role')roleSymbols.add(symbolAt(element.name));
});
walk(source(windowRoot),node=>{
  if(ts.isBinaryExpression(node)&&[ts.SyntaxKind.EqualsEqualsEqualsToken,ts.SyntaxKind.EqualsEqualsToken,ts.SyntaxKind.ExclamationEqualsEqualsToken,ts.SyntaxKind.ExclamationEqualsToken].includes(node.operatorToken.kind)){
    for(const [role,value] of [[node.left,node.right],[node.right,node.left]])if([...roleSymbols].some(symbol=>stateReference(role,symbol))){const literal=literalString(value);if(literal!==undefined)windowRoles.add(literal);else unknown('window role',windowRoot,node,'non-literal role comparison');}
  }
  if(ts.isSwitchStatement(node)&&[...roleSymbols].some(symbol=>stateReference(node.expression,symbol)))for(const clause of node.caseBlock.clauses)if(ts.isCaseClause(clause)){const literal=literalString(clause.expression);if(literal!==undefined)windowRoles.add(literal);else unknown('window role',windowRoot,clause,'non-literal role case');}
  if((ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))&&[...roleSymbols].some(symbol=>dependsOn(node.tagName,symbol)))unknown('window role',windowRoot,node,'dynamic role-selected component');
  // Resolve the actual fallback declaration, including import aliases.
  if(ts.isConditionalExpression(node)&&ts.isJsxSelfClosingElement(node.whenFalse)){
    const name=componentSymbol(node.whenFalse.tagName)?.getName();if(name?.endsWith('Window'))windowRoles.add(name.replace(/Window$/,'').toLowerCase());
  }
});
// Surface predicates may be arbitrarily named; follow their bound initializers.
const UNKNOWN=Symbol('unknown'),DEFAULT_SURFACE=Symbol('unmatched surface');
function isSurfaceRead(node){return ts.isCallExpression(node)&&ts.isPropertyAccessExpression(node.expression)&&node.expression.name.text==='get'&&node.arguments.length===1&&literalString(node.arguments[0])==='surface';}
function surfaceDependent(node,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node))return false;seen.add(node);if(isSurfaceRead(node))return true;
  const initial=ts.isIdentifier(node)?initializer(node):undefined;if(initial&&surfaceDependent(initial,seen))return true;
  if(ts.isCallExpression(node)){const declaration=valueDeclaration(node.expression);if(declaration&&ts.isFunctionLike(declaration)&&declaration.body&&declaration.getSourceFile()===source(bootstrap)&&surfaceDependent(declaration.body,seen))return true;}
  return ts.forEachChild(node,child=>surfaceDependent(child,seen))===true;
}
function evaluate(node,surface,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node))return UNKNOWN;seen=new Set(seen).add(node);
  if(isSurfaceRead(node))return surface;
  if(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node))return node.text;
  if(node.kind===ts.SyntaxKind.TrueKeyword)return true;if(node.kind===ts.SyntaxKind.FalseKeyword)return false;
  if(node.kind===ts.SyntaxKind.NullKeyword)return null;
  if(ts.isNumericLiteral(node))return Number(node.text);
  if(ts.isArrowFunction(node)||ts.isFunctionExpression(node)){
    if(node.parameters.length)return UNKNOWN;
    if(!ts.isBlock(node.body))return evaluate(node.body,surface,seen);
    if(node.body.statements.length===1&&ts.isReturnStatement(node.body.statements[0]))return evaluate(node.body.statements[0].expression,surface,seen);
    return UNKNOWN;
  }
  if(ts.isIdentifier(node)){const initial=initializer(node);return initial?evaluate(initial,surface,seen):UNKNOWN;}
  if(ts.isPrefixUnaryExpression(node)&&node.operator===ts.SyntaxKind.ExclamationToken){const value=evaluate(node.operand,surface,seen);return value===UNKNOWN?UNKNOWN:!value;}
  if(ts.isBinaryExpression(node)){
    const left=evaluate(node.left,surface,seen),right=evaluate(node.right,surface,seen);if(left===UNKNOWN||right===UNKNOWN)return UNKNOWN;
    switch(node.operatorToken.kind){case ts.SyntaxKind.EqualsEqualsEqualsToken:case ts.SyntaxKind.EqualsEqualsToken:return left===right;case ts.SyntaxKind.ExclamationEqualsEqualsToken:case ts.SyntaxKind.ExclamationEqualsToken:return left!==right;case ts.SyntaxKind.AmpersandAmpersandToken:return left&&right;case ts.SyntaxKind.BarBarToken:return left||right;case ts.SyntaxKind.QuestionQuestionToken:return left??right;}
  }
  if(ts.isConditionalExpression(node)){const condition=evaluate(node.condition,surface,seen);return condition===UNKNOWN?UNKNOWN:evaluate(condition?node.whenTrue:node.whenFalse,surface,seen);}
  return UNKNOWN;
}
walk(source(bootstrap),node=>{
  if(ts.isBinaryExpression(node)&&[ts.SyntaxKind.EqualsEqualsEqualsToken,ts.SyntaxKind.EqualsEqualsToken,ts.SyntaxKind.ExclamationEqualsEqualsToken,ts.SyntaxKind.ExclamationEqualsToken].includes(node.operatorToken.kind)){
    for(const [query,value] of [[node.left,node.right],[node.right,node.left]])if(surfaceDependent(query)){const literal=evaluate(value,DEFAULT_SURFACE);if(typeof literal==='string')surfaceValues.add(literal);}
  }
  if(ts.isSwitchStatement(node)&&surfaceDependent(node.expression))for(const clause of node.caseBlock.clauses)if(ts.isCaseClause(clause)){const value=evaluate(clause.expression,DEFAULT_SURFACE);if(typeof value==='string')surfaceValues.add(value);else unknown('surface',bootstrap,clause,'non-literal case');}
});
const renderCalls=[];walk(source(bootstrap),node=>{if(ts.isCallExpression(node)&&ts.isPropertyAccessExpression(node.expression)&&node.expression.name.text==='render'&&node.arguments.length)renderCalls.push(node);});
function surfaceRender(surface){
  const roots=new Set(),trace=[];
  function visit(node,seen=new Set()){
    node=unwrap(node);if(!node||seen.has(node))return;seen=new Set(seen).add(node);
    if(ts.isIdentifier(node)){const initial=initializer(node);if(initial){visit(initial,seen);return;}}
    if(ts.isConditionalExpression(node)&&surfaceDependent(node.condition)){
      const condition=evaluate(node.condition,surface);if(condition===UNKNOWN){unknown('surface',bootstrap,node.condition,'unresolved render predicate');return;}
      trace.push(node.pos+':'+Boolean(condition));visit(condition?node.whenTrue:node.whenFalse,seen);return;
    }
    if(ts.isBinaryExpression(node)&&[ts.SyntaxKind.AmpersandAmpersandToken,ts.SyntaxKind.BarBarToken].includes(node.operatorToken.kind)&&surfaceDependent(node.left)){
      const condition=evaluate(node.left,surface);if(condition===UNKNOWN){unknown('surface',bootstrap,node.left,'unresolved short-circuit render predicate');return;}
      trace.push(node.pos+':'+Boolean(condition));if(node.operatorToken.kind===ts.SyntaxKind.AmpersandAmpersandToken?condition:!condition)visit(node.right,seen);return;
    }
    if((ts.isCallExpression(node)||ts.isElementAccessExpression(node))&&surfaceDependent(node)){unknown('surface',bootstrap,node,'unresolved dynamic render selection');return;}
    if(ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node)){
      if(surfaceDependent(node.tagName))unknown('surface',bootstrap,node,'dynamic component selected by query');
      const declaration=componentDeclaration(node.tagName);if(declaration){const file=relative(declaration.getSourceFile().fileName);if(file.startsWith('apps/desktop/src/'))roots.add(file);}
      else if(/^[A-Z]/.test(node.tagName.getText())&&!reactImport(bootstrap,node.tagName))unknown('surface component',bootstrap,node,node.tagName.getText());
    }
    ts.forEachChild(node,child=>visit(child,seen));
  }
  for(const call of renderCalls){
    let enabled=true;
    for(let parent=call.parent,child=call;parent;child=parent,parent=parent.parent)if(ts.isIfStatement(parent)&&surfaceDependent(parent.expression)){
      const condition=evaluate(parent.expression,surface);if(condition===UNKNOWN){unknown('surface',bootstrap,parent.expression,'unresolved render guard');enabled=false;break;}
      const selected=child===parent.thenStatement?Boolean(condition):!condition;trace.push(parent.pos+':'+Boolean(condition));if(!selected){enabled=false;break;}
    }
    if(enabled)visit(call.arguments[0]);
  }
  return {roots:[...roots].sort(),trace:trace.sort().join('|')};
}
const defaultRender=surfaceRender(DEFAULT_SURFACE);
if(defaultRender.trace&&surfaceValues.size===0)unknown('surface',bootstrap,source(bootstrap),'query-selected render has no statically enumerable values');
const pages=[];
for(const page of mainPages)pages.push({id:'main:'+page,kind:'main_page',role:'main',route:page,entry_roots:['apps/desktop/src/windows/main/MainWindow.tsx']});
for(const page of settingsPages)pages.push({id:'settings:'+page,kind:'settings_page',role:'settings',route:page,entry_roots:[settings]});
for(const role of [...windowRoles].sort()){
  const expected=`apps/desktop/src/windows/${role}/${role[0].toUpperCase()+role.slice(1)}Window.tsx`,known=fs.existsSync(path.join(root,expected));
  if(!known)unknown('window role',windowRoot,source(windowRoot),'role='+role+' requires explicit window entry mapping');
  pages.push({id:'window:'+role,kind:'window',role,route:role,entry_roots:[known?expected:windowRoot]});
}
for(const surface of [...surfaceValues].sort()){
  const rendered=surfaceRender(surface),fallbackOnly=rendered.trace===defaultRender.trace;
  if(fallbackOnly||!rendered.roots.length)unknown('surface',bootstrap,source(bootstrap),'surface='+surface+' has no resolved distinct render branch');
  // Include only local branch roots; external wrappers are not product surfaces.
  pages.push({id:'surface:'+surface,kind:'transient_surface',role:surface,route:'surface='+surface,entry_roots:fallbackOnly?[]:rendered.roots});
}
const files=[];function list(dir){for(const f of fs.readdirSync(path.join(root,dir),{withFileTypes:true})){const p=dir+'/'+f.name;if(f.isDirectory())list(p);else if(/\.[jt]sx$/.test(f.name))files.push(p);}}
list('apps/desktop/src/features');list('apps/desktop/src/windows');
const overlaySymbols=new Map([
  [resolvedExport('apps/desktop/src/ui/primitives/Controls.tsx','Sheet'),'Sheet'],
  [resolvedExport('apps/desktop/src/ui/primitives/Overlays.tsx','ActionMenu'),'ActionMenu'],
  [resolvedExport('apps/desktop/src/ui/primitives/Overlays.tsx','AnchoredPopover'),'AnchoredPopover']
].filter(([key])=>key));
function overlayTag(node){return overlaySymbols.get(identity(componentSymbol(node)));}
const overlays=[];
for(const file of files){let index=0;walk(source(file),n=>{
  if(!(ts.isJsxOpeningElement(n)||ts.isJsxSelfClosingElement(n)))return;
  const tag=overlayTag(n.tagName);if(!tag){
    const declaration=componentDeclaration(n.tagName),local=declaration&&relative(declaration.getSourceFile().fileName).startsWith('apps/desktop/src/');
    const value=declaration&&ts.isVariableDeclaration(declaration)?unwrap(declaration.initializer):declaration;
    if(/^[A-Z]/.test(n.tagName.getText())&&local&&!(value&&(ts.isFunctionDeclaration(value)||ts.isClassDeclaration(value)||ts.isArrowFunction(value)||ts.isFunctionExpression(value)||ts.isCallExpression(value))))unknown('rendered component',file,n,'unresolved value for '+n.tagName.getText());
    if(/^[A-Z]/.test(n.tagName.getText())&&!declaration){
      // Only an actual React import can justify an unresolved external wrapper.
      if(!reactImport(file,n.tagName))unknown('rendered component',file,n,n.tagName.getText());
    }
    return;
  }
  index++;const attributes=Object.fromEntries(n.attributes.properties.filter(ts.isJsxAttribute).map(a=>[a.name.text,a.initializer?.getText()||'true']));const title=attributes.title||attributes.label||'',stable=tag+':'+(attributes.open||'')+':'+(title||String(index));const id='overlay:'+path.basename(file,'.tsx')+':'+hash(stable).slice(0,10);overlays.push({id,kind:'overlay',role:'container',route:stable,entry_roots:[file],tag,attributes});
});}
pages.push(...overlays);
function literalUnion(node){
  if(!node)return [];const type=checker.getTypeFromTypeNode(node),members=type.isUnion()?type.types:[type];
  return members.length&&members.every(value=>value.flags&(ts.TypeFlags.StringLiteral|ts.TypeFlags.NumberLiteral))?members.map(value=>value.value):[];
}
function arrayChoices(node,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node))return [];seen.add(node);if(ts.isIdentifier(node)){const initial=initializer(node);if(initial&&initial!==node)return arrayChoices(initial,seen);}
  if(!node||!ts.isArrayLiteralExpression(node))return [];
  const values=node.elements.map(element=>{
    element=unwrap(element);if(ts.isStringLiteral(element))return element.text;if(ts.isNumericLiteral(element))return Number(element.text);
    if(ts.isArrayLiteralExpression(element)){const first=element.elements[0];if(first&&ts.isStringLiteral(first))return first.text;if(first&&ts.isNumericLiteral(first))return Number(first.text);}
    if(ts.isObjectLiteralExpression(element)){const id=element.properties.find(p=>ts.isPropertyAssignment(p)&&p.name.getText()==='id');if(id&&ts.isStringLiteral(id.initializer))return id.initializer.text;if(id&&ts.isNumericLiteral(id.initializer))return Number(id.initializer.text);}
    return null;
  });return values.every(value=>value!==null)?values:[];
}
function eventAttribute(node){let parent=node.parent;while(parent&&!ts.isJsxAttribute(parent))parent=parent.parent;return parent&&['onClick','onPress','onKeyDown'].includes(parent.name.text)?parent:undefined;}
function isUseState(node){
  const symbol=symbolAt(node);if(symbol?.getName()==='useState')return true;
  // Missing external type declarations must not hide the imported hook identity.
  const local=ts.isPropertyAccessExpression(node)?node.expression.getText():node.getText(),name=ts.isPropertyAccessExpression(node)?node.name.text:undefined;
  return node.getSourceFile().statements.some(statement=>ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text==='react'&&statement.importClause?.namedBindings&&(ts.isNamedImports(statement.importClause.namedBindings)?statement.importClause.namedBindings.elements.some(item=>item.name.text===local&&(item.propertyName?.text??item.name.text)==='useState'):statement.importClause.namedBindings.name.text===local&&name==='useState'));
}
function reachableEventCalls(file){
  const calls=new Set(),references=new Set(),seen=new Set();
  function visit(node){
    if(!node||seen.has(node))return;seen.add(node);
    if(ts.isCallExpression(node))calls.add(node);
    if(ts.isIdentifier(node)){
      references.add(symbolAt(node));const declaration=valueDeclaration(node);
      if(declaration?.getSourceFile()===source(file)){
        const handler=ts.isVariableDeclaration(declaration)?unwrap(declaration.initializer):declaration;
        if(handler&&(ts.isArrowFunction(handler)||ts.isFunctionExpression(handler)||ts.isFunctionDeclaration(handler))&&handler.body){
          const body=handler.body;
          // Follow simple named navigation callbacks, not unrelated business workflows.
          if(!ts.isBlock(body)||body.statements.length===1&&(ts.isExpressionStatement(body.statements[0])||ts.isReturnStatement(body.statements[0])))visit(body);
        }
      }
    }
    ts.forEachChild(node,visit);
  }
  walk(source(file),node=>{if(ts.isJsxAttribute(node)&&['onClick','onPress','onKeyDown'].includes(node.name.text))visit(node.initializer);});
  return {calls,references};
}
function stateReference(node,state,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node)||!ts.isIdentifier(node))return false;seen.add(node);
  if(symbolAt(node)===state)return true;const initial=initializer(node);return initial?stateReference(initial,state,seen):false;
}
function selectionPredicate(node,state,seen=new Set()){
  node=unwrap(node);if(!node||seen.has(node))return false;seen.add(node);
  if(ts.isIdentifier(node)){const initial=initializer(node);return initial?selectionPredicate(initial,state,seen):symbolAt(node)===state;}
  if(ts.isPrefixUnaryExpression(node))return selectionPredicate(node.operand,state,seen);
  if(ts.isConditionalExpression(node))return selectionPredicate(node.condition,state,seen);
  if(ts.isBinaryExpression(node)){
    if([ts.SyntaxKind.EqualsEqualsEqualsToken,ts.SyntaxKind.EqualsEqualsToken,ts.SyntaxKind.ExclamationEqualsEqualsToken,ts.SyntaxKind.ExclamationEqualsToken].includes(node.operatorToken.kind))return stateReference(node.left,state)||stateReference(node.right,state);
    if([ts.SyntaxKind.AmpersandAmpersandToken,ts.SyntaxKind.BarBarToken].includes(node.operatorToken.kind))return selectionPredicate(node.left,state,seen)||selectionPredicate(node.right,state,seen);
  }
  return false;
}
function renderedStateBranch(node,state){
  if(!ts.isConditionalExpression(node)&&!ts.isIfStatement(node)&&!ts.isSwitchStatement(node)&&!(ts.isBinaryExpression(node)&&[ts.SyntaxKind.AmpersandAmpersandToken,ts.SyntaxKind.BarBarToken].includes(node.operatorToken.kind)))return false;
  const condition=ts.isConditionalExpression(node)?node.condition:ts.isIfStatement(node)||ts.isSwitchStatement(node)?node.expression:node.left;
  if(ts.isSwitchStatement(node)?!stateReference(condition,state):!selectionPredicate(condition,state))return false;
  // A form's optional slot is not itself a page route. Do not inspect through props.
  for(let parent=node.parent;parent&&!ts.isFunctionLike(parent);parent=parent.parent)if(ts.isJsxAttribute(parent))return false;
  return contains(node,n=>ts.isJsxElement(n)||ts.isJsxSelfClosingElement(n)||ts.isJsxFragment(n));
}
for(const file of files.filter(p=>p.includes('/features/'))){const reachable=reachableEventCalls(file);walk(source(file),n=>{
  if(!ts.isVariableDeclaration(n)||!ts.isArrayBindingPattern(n.name)||!n.initializer||!ts.isCallExpression(n.initializer)||!isUseState(n.initializer.expression))return;
  const stateBinding=n.name.elements[0]?.name,setterBinding=n.name.elements[1]?.name;if(!stateBinding||!setterBinding||!ts.isIdentifier(stateBinding)||!ts.isIdentifier(setterBinding))return;
  const state=symbolAt(stateBinding),setter=symbolAt(setterBinding),stateName=stateBinding.text;
  const declaredValues=literalUnion(n.initializer.typeArguments?.[0]),initial=evaluate(n.initializer.arguments[0],DEFAULT_SURFACE);
  if(!declaredValues.length&&initial!==UNKNOWN&&!['string','number'].includes(typeof initial))return;
  const directChoices=new Set(),mappedChoices=new Set();let navigation=false,renderBranch=false,unresolvedChoice=false;
  walk(source(file),node=>{
    if(renderedStateBranch(node,state))renderBranch=true;
    if((ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))&&dependsOn(node.tagName,state)){renderBranch=true;unknown('navigation',file,node,'dynamic component for '+stateName);}
    if(ts.isArrayLiteralExpression(node)&&declaredValues.length>=2){const ids=arrayChoices(node);if(ids.length>=2&&ids.every(value=>declaredValues.includes(value)))navigation=true;}
    if(ts.isJsxAttribute(node)&&node.name.text==='title'&&overlayTag(node.parent?.parent?.tagName)==='Sheet'&&node.initializer&&dependsOn(node.initializer,state))navigation=true;
    if(!ts.isCallExpression(node)||symbolAt(node.expression)!==setter||!(eventAttribute(node)||!declaredValues.length&&reachable.calls.has(node)))return;
    const value=evaluate(node.arguments[0],DEFAULT_SURFACE);
    if(typeof value==='string'||typeof value==='number'){directChoices.add(value);return;}
    let choices=[];
    for(let parent=node.parent;parent;parent=parent.parent)if(ts.isCallExpression(parent)&&ts.isPropertyAccessExpression(parent.expression)&&parent.expression.name.text==='map'&&parent.arguments.some(argument=>argument===node||contains(argument,child=>child===node))){choices=arrayChoices(parent.expression.expression);if(choices.length)break;}
    if(choices.length)choices.forEach(choice=>mappedChoices.add(choice));else unresolvedChoice=true;
  });
  let values=declaredValues;
  const navValues=[...new Set([...directChoices,...mappedChoices])];
  if(!values.length){
    // Inferred finite states require actual navigational handlers plus rendered content.
    if(!renderBranch||!navValues.length&&!unresolvedChoice&&!reachable.references.has(setter))return;
    if(initial===UNKNOWN){unknown('navigation',file,n,'unresolved initial value for '+stateName);return;}
    if(!navValues.length&&reachable.references.has(setter))unresolvedChoice=true;
    if(unresolvedChoice){unknown('navigation',file,n,'non-enumerable choices for '+stateName);return;}
    if(navValues.length<2)return;
    values=[...new Set([initial,...navValues])];navigation=true;
  }else if(directChoices.size>=2&&values.every(value=>directChoices.has(value))||mappedChoices.size>=2&&values.every(value=>mappedChoices.has(value)))navigation=true;
  if(!navigation||values.length<2)return;
  if(unresolvedChoice&&renderBranch)unknown('navigation',file,n,'partially unresolved choices for '+stateName);
  if(navValues.length&&JSON.stringify([...navValues].sort())!==JSON.stringify([...values].sort()))unknown('navigation',file,n,'Subview union/navigation mismatch for '+stateName);
  for(const value of values)pages.push({id:'subpage:'+path.basename(file,'.tsx')+':'+stateName+':'+value,kind:'subpage',role:'container',route:String(value),entry_roots:[file],state_variable:stateName,navigation_values:navValues});
});}

// Inventory the actual UI tree; new exported components must require mapping.
const sharedFiles=[];
function listShared(dir){for(const item of fs.readdirSync(path.join(root,dir),{withFileTypes:true})){const file=dir+'/'+item.name;if(item.isDirectory())listShared(file);else if(/\.[jt]sx?$/.test(item.name))sharedFiles.push(file);}}
listShared('apps/desktop/src/ui');
const sharedIdentities=new Set(),sharedModules=new Set();
function inventoryShared(file,exports){for(let symbol of exports){
  if(symbol.declarations?.every(declaration=>ts.isExportSpecifier(declaration)&&(declaration.isTypeOnly||declaration.parent.parent.isTypeOnly)))continue;
  const exposedName=symbol.getName();if(symbol.flags&ts.SymbolFlags.Alias)symbol=checker.getAliasedSymbol(symbol);
  if(symbol.flags&ts.SymbolFlags.Module){const moduleIdentity=identity(symbol);if(!sharedModules.has(moduleIdentity)){sharedModules.add(moduleIdentity);inventoryShared(file,checker.getExportsOfModule(symbol));}continue;}
  if(!(symbol.flags&ts.SymbolFlags.Value))continue;
  const declaration=symbol.valueDeclaration??symbol.declarations?.find(node=>!ts.isImportSpecifier(node)&&!ts.isExportSpecifier(node));
  const canonicalName=symbol.getName()==='default'?path.basename(file).replace(/\.tsx?$/,''):symbol.getName();
  if(!/^[A-Z]/.test(canonicalName)&&!/^[A-Z]/.test(exposedName)&&exposedName!=='default')continue;
  if(!declaration){unknown('shared export',file,source(file),exposedName);continue;}
  const origin=relative(declaration.getSourceFile().fileName),key=identity(symbol);if(sharedIdentities.has(key))continue;
  let value=ts.isVariableDeclaration(declaration)?unwrap(declaration.initializer):ts.isExportAssignment(declaration)?unwrap(declaration.expression):declaration;
  const component=value&&(ts.isFunctionDeclaration(value)||ts.isClassDeclaration(value)||ts.isArrowFunction(value)||ts.isFunctionExpression(value)||ts.isCallExpression(value));
  if(!component){unknown('shared export',file,declaration,exposedName+' is not a statically resolved component');continue;}
  if(!origin.startsWith('apps/desktop/src/ui/')){unknown('shared export',file,declaration,exposedName+' has a non-UI declaration');continue;}
  sharedIdentities.add(key);const name=/^[A-Z]/.test(canonicalName)?canonicalName:exposedName;
  pages.push({id:'shared:'+name,kind:'shared_component',role:'container',route:name,entry_roots:[origin]});
}}
for(const file of sharedFiles)inventoryShared(file,exportedSymbols(file));
for(const file of sourceFiles)for(const diagnostic of source(file).parseDiagnostics??[])errors.push('Unknown source syntax: '+file+': '+ts.flattenDiagnosticMessageText(diagnostic.messageText,' '));
if(JSON.stringify(mainPages)!==JSON.stringify(mainNav))errors.push('MainPage union and actual mainNavigation differ');
if(JSON.stringify(settingsPages)!==JSON.stringify(settingsNav))errors.push('SettingsPage union and actual settings destinations differ');
if(new Set(pages.map(p=>p.id)).size!==pages.length)errors.push('Duplicate discovered surface identity');
const globalRoots=['apps/desktop/src/ui/reset.css','apps/desktop/src/ui/tokens/generated.css','apps/desktop/src/ui/tokens/tokens.json'];
for(const page of pages){page.dependency_paths=closure([...page.entry_roots,...globalRoots]);page.dependency_sha256=Object.fromEntries(page.dependency_paths.map(p=>[p,hash(fs.readFileSync(checked(p)))]));}
const report={schema_version:1,parser:'typescript-ast',main_pages:mainPages,main_navigation:mainNav,settings_pages:settingsPages,settings_navigation:settingsNav,window_roles:[...windowRoles].sort(),transient_surfaces:[...surfaceValues].sort(),pages:pages.sort((a,b)=>a.id.localeCompare(b.id)),errors:[...new Set(errors)]};
if(args.includes('--production-dependencies')){
  // Optional impact evidence only: preserve the frozen inventory's default
  // shape and page-specific closures. The real bootstrap can reach resources
  // outside src (including a file whose name happens to say test or docs).
  const seen=new Set();
  function dependency(from,spec,node){
    const local=spec.startsWith('.')?resolveImport(from,spec):spec.startsWith('/src/')?resolveImport('apps/desktop/index.html','.'+spec):null;
    if(local){visit(local);return;}
    if(spec.startsWith('.')||spec.startsWith('/'))unknown('production dependency',from,node,'unresolved local import '+spec);
  }
  function visit(file){
    if(seen.has(file))return;seen.add(file);checked(file);
    if(/\.[jt]sx?$/.test(file)){
      const sf=source(file);
      for(const diagnostic of sf.parseDiagnostics??[])errors.push('Unknown production source syntax: '+file+': '+ts.flattenDiagnosticMessageText(diagnostic.messageText,' '));
      walk(sf,node=>{
        if((ts.isImportDeclaration(node)||ts.isExportDeclaration(node))&&node.moduleSpecifier){
          if(ts.isStringLiteral(node.moduleSpecifier))dependency(file,node.moduleSpecifier.text,node);
          else unknown('production dependency',file,node,'non-literal module import');
        }
        if(ts.isCallExpression(node)&&(node.expression.kind===ts.SyntaxKind.ImportKeyword||ts.isIdentifier(node.expression)&&node.expression.text==='require')){
          const spec=literalString(node.arguments[0]);
          if(spec!==undefined)dependency(file,spec,node);else unknown('production dependency',file,node,'non-enumerable dynamic module import');
        }
        if(ts.isCallExpression(node)&&ts.isPropertyAccessExpression(node.expression)&&node.expression.expression.getText()==='import.meta')unknown('production dependency',file,node,'non-enumerable import.meta module selection');
        if(ts.isNewExpression(node)&&ts.isIdentifier(node.expression)&&node.expression.text==='URL'&&node.arguments?.[1]?.getText()==='import.meta.url'){
          const spec=literalString(node.arguments[0]);
          if(spec!==undefined)dependency(file,spec,node);else unknown('production dependency',file,node,'non-enumerable import.meta URL');
        }
      });
    }else if(file.endsWith('.css')){
      const css=read(file);
      for(const match of css.matchAll(/(?:@import\s+['"]([^'"]+)['"]|url\(\s*['"]?([^)'"\s]+)['"]?\s*\))/g)){
        const spec=match[1]??match[2];if(/^(?:data:|https?:|#)/.test(spec))continue;
        const local=resolveImport(file,spec.startsWith('.')?spec:'./'+spec);
        if(local)visit(local);else errors.push('Unknown production CSS dependency: '+file+': '+spec);
      }
    }
  }
  report.production_entry_roots=[bootstrap];
  report.production_entry_roots.forEach(visit);
  report.production_dependency_paths=[...seen].sort();
  report.errors=[...new Set(errors)];
}
console.log(JSON.stringify(report,null,2));
