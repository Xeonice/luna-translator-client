#!/usr/bin/env node
/**
 * Syntax-aware enforcement of engineering-layout.md's frontend boundaries.
 * Requires the repository's already installed TypeScript; installs nothing.
 * This checks imports and statically identifiable capabilities, not arbitrary
 * runtime behavior. It deliberately has no grandfathering/ignore mechanism.
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const ts = require(path.join(repositoryRoot, 'apps/desktop/node_modules/typescript'));
const SOURCE_DIRECTORY = 'apps/desktop/src';
const TAURI_GATEWAYS = new Set(['bridge/tauri-port.ts']);
const BRIDGE_PUBLIC_FILES = new Set(['bridge/application-port.ts', 'bridge/contracts.generated.ts', 'bridge/projections/index.ts']);
const COMPOSITION_ROOTS = new Set(['app/bootstrap.tsx']);
const SOURCE_LAYERS = new Set(['app', 'bridge', 'windows', 'features', 'ui', 'i18n']);
const GLOBAL_OBJECTS = new Set(['window', 'globalThis', 'self']);
const GLOBAL_CAPABILITIES = new Map([
  ['fetch', 'FE_HTTP_ACCESS'], ['XMLHttpRequest', 'FE_HTTP_ACCESS'],
  ['WebSocket', 'FE_HTTP_ACCESS'], ['EventSource', 'FE_HTTP_ACCESS'],
  ['indexedDB', 'FE_DATABASE_ACCESS'], ['openDatabase', 'FE_DATABASE_ACCESS'],
  ['__TAURI__', 'FE_TAURI_GATEWAY'], ['__TAURI_INTERNALS__', 'FE_TAURI_GATEWAY'],
  ['eval', 'FE_DYNAMIC_CODE'], ['Function', 'FE_DYNAMIC_CODE'],
]);
const FORBIDDEN_PACKAGES = [
  ['FE_HTTP_ACCESS', ['axios', 'ky', 'got', 'undici', 'node-fetch', 'cross-fetch', 'superagent', 'socket.io-client', 'ws', 'openai', '@anthropic-ai/sdk', '@google/generative-ai']],
  ['FE_DATABASE_ACCESS', ['sqlite3', 'better-sqlite3', 'sql.js', '@sqlite.org/sqlite-wasm', '@libsql/client', 'dexie', 'idb', 'kysely', 'drizzle-orm', '@prisma/client']],
  ['FE_FFI_ACCESS', ['ffi-napi', 'ref-napi', 'koffi', 'bun:ffi']],
  ['FE_CREDENTIAL_ACCESS', ['keytar', 'keyring', '@napi-rs/keyring', '@tauri-apps/plugin-stronghold']],
];
const NODE_IO = /^(?:node:)?(?:fs|fs\/promises|http|https|http2|net|tls|dgram|dns|child_process|sqlite)(?:\/|$)/;
const SOURCE_EXTENSIONS = /\.(?:[cm]?[jt]s|[jt]sx)$/;

export const RULES = Object.freeze({
  FE_HTTP_ACCESS: 'HTTP/网络必须经 Rust 应用用例，前端不得直接使用网络客户端。',
  FE_DATABASE_ACCESS: '数据库必须经 Rust 业务端口，前端不得自行保存业务数据库。',
  FE_FFI_ACCESS: 'FFI 仅限原生明确模块，前端不得直接绑定。',
  FE_CREDENTIAL_ACCESS: '系统秘密仅由 Rust CredentialVault 解析，WebView 不接触凭据 API。',
  FE_NODE_IO: '产品前端不得直接调用 Node 系统 I/O。',
  FE_TAURI_GATEWAY: 'Tauri API 仅允许在 bridge/tauri-port.ts 内导入或引用。',
  FE_BRIDGE_PRIVATE: '业务 UI 只消费 bridge 的公开端口、契约及投影入口；TauriPort 由 bootstrap 装配。',
  FE_UI_BUSINESS_IMPORT: '公共 UI 不得导入 app、bridge、windows 或 features。',
  FE_UI_LAYER_ORDER: 'tokens/icons/primitives/patterns 不得反向依赖更高 UI 层。',
  FE_FEATURE_UPWARD_IMPORT: 'feature 不得反向依赖 app 或 windows。',
  FE_FEATURE_PRIVATE_IMPORT: '跨 feature 只能导入该 feature 的 index.ts 公开接口。',
  FE_BRIDGE_UI_IMPORT: 'bridge 不得反向导入 app、windows、features 或 ui。',
  FE_ARIA_WRAPPER: 'React Aria 交互库仅允许 ui/primitives 的窄封装直接导入。',
  FE_PRODUCTION_TEST_IMPORT: '产品 src 不得导入测试、fixture 或设计演示源码。',
  FE_SOURCE_ESCAPE: '产品本地导入不得逃逸 src 边界；外部依赖使用锁定 npm 包。',
  FE_SOURCE_LAYOUT: '产品源码必须位于 app/bridge/windows/features/ui/i18n 目标职责目录内。',
  FE_UNCLASSIFIED_IMPORT: '目标结构内的模块不得导入尚未归层的旧根文件，以免绕过依赖方向。',
  FE_UNRESOLVED_LOCAL_IMPORT: '本地或已配置 alias 的导入必须可解析，不能借不存在目标绕过边界。',
  FE_DYNAMIC_MODULE: '动态 import/require 必须使用静态字面量；import.meta.glob 不在允许边界内。',
  FE_DYNAMIC_CODE: '产品前端不得使用 eval/Function 生成不可分析的代码。',
  FE_PARSE_ERROR: '语法错误导致边界分析不完整，必须先修复。',
});

export const LIMITATIONS = Object.freeze([
  '检查 src 的 TS/TSX/JS/JSX/MTS/CTS 文件及静态 import、export-from、import-type、动态 import、require、import-equals；解析 tsconfig paths/baseUrl。',
  '识别未被本地变量遮蔽的全局网络/数据库 API、window/globalThis/self 属性、常量属性别名及解构；不执行源码。',
  '动态模块路径、eval/Function 和 glob 被拒绝；不证明第三方包内部行为、任意运行时对象流或 Rust 侧权限隔离。',
  '不检查 CSS 视觉差异、真实桌面材质、产品功能或编译类型正确性；这些由独立门禁验证。',
]);

const slash = value => value.split(path.sep).join('/');
const inside = (candidate, directory) => candidate === directory || candidate.startsWith(`${directory}${path.sep}`);
const packageMatches = (specifier, name) => specifier === name || specifier.startsWith(`${name}/`);

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap(entry => {
    const target = path.join(directory, entry.name);
    // A source symlink would make the inventory dependent on an external tree.
    if (entry.isSymbolicLink()) throw new Error(`Source symlink is not supported: ${entry.name}`);
    if (entry.isDirectory()) return walk(target);
    return SOURCE_EXTENSIONS.test(entry.name) ? [target] : [];
  });
}

function stripExpression(node) {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isTypeAssertionExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
}

function literal(node) {
  node = stripExpression(node);
  return node && (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) ? node.text : undefined;
}

function property(node) {
  if (ts.isPropertyAccessExpression(node)) return { object: node.expression, name: node.name.text };
  if (ts.isElementAccessExpression(node)) return { object: node.expression, name: literal(node.argumentExpression) };
  return undefined;
}

function readOptions(root) {
  const desktop = path.join(root, 'apps/desktop');
  const configPath = path.join(desktop, 'tsconfig.json');
  const defaults = { allowJs: true, noEmit: true, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX };
  if (!fs.existsSync(configPath)) return defaults;
  const config = ts.readConfigFile(configPath, ts.sys.readFile);
  if (config.error) throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, ' '));
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, desktop);
  // TS18003 (empty include) is irrelevant: the guard inventories src itself.
  const errors = parsed.errors.filter(error => error.code !== 18003);
  if (errors.length) throw new Error(errors.map(error => ts.flattenDiagnosticMessageText(error.messageText, ' ')).join('; '));
  return { ...defaults, ...parsed.options, allowJs: true, noEmit: true };
}

function resolveImport(specifier, importer, options, sourceRoot) {
  const result = ts.resolveModuleName(specifier, importer, options, ts.sys).resolvedModule;
  if (result) return path.resolve(result.resolvedFileName);
  // The TS resolver intentionally omits CSS and other non-code resources.
  const bases = [];
  if (specifier.startsWith('.')) bases.push(path.resolve(path.dirname(importer), specifier));
  if (path.isAbsolute(specifier)) bases.push(path.resolve(specifier));
  for (const [alias, targets] of Object.entries(options.paths ?? {})) {
    const wildcard = alias.indexOf('*');
    const matches = wildcard < 0 ? specifier === alias : specifier.startsWith(alias.slice(0, wildcard)) && specifier.endsWith(alias.slice(wildcard + 1));
    if (!matches) continue;
    const replacement = wildcard < 0 ? '' : specifier.slice(wildcard, specifier.length - alias.length + wildcard + 1);
    for (const target of targets) bases.push(path.resolve(options.baseUrl ?? options.pathsBasePath ?? sourceRoot, target.replace('*', replacement)));
  }
  for (const base of bases) {
    for (const candidate of [base, ...['.ts', '.tsx', '.js', '.jsx', '.json', '/index.ts', '/index.tsx'].map(extension => `${base}${extension}`)]) {
      if (fs.existsSync(candidate) && fs.statSync(candidate).isFile()) return candidate;
    }
  }
  return undefined;
}

function isConfiguredLocal(specifier, options) {
  if (specifier.startsWith('.') || path.isAbsolute(specifier) || /^(?:@|~)\//.test(specifier)) return true;
  return Object.keys(options.paths ?? {}).some(alias => {
    const [prefix, suffix = ''] = alias.split('*');
    return alias.includes('*') ? specifier.startsWith(prefix) && specifier.endsWith(suffix) : alias === specifier;
  });
}

export function analyzeFrontend({ root = repositoryRoot } = {}) {
  root = path.resolve(root);
  const sourceRoot = path.join(root, SOURCE_DIRECTORY);
  if (!fs.existsSync(sourceRoot)) throw new Error(`Missing required source directory: ${SOURCE_DIRECTORY}`);
  const files = walk(sourceRoot);
  if (!files.length) throw new Error(`No analyzable source files: ${SOURCE_DIRECTORY}`);
  const options = readOptions(root);
  const program = ts.createProgram(files, options);
  const checker = program.getTypeChecker();
  const findings = [];
  const seen = new Set();

  for (const filename of files) {
    const source = program.getSourceFile(filename);
    if (!source) throw new Error('TypeScript did not parse an inventoried source file');
    const relative = slash(path.relative(sourceRoot, filename));
    const layer = relative.split('/')[0];
    const feature = relative.startsWith('features/') ? relative.split('/')[1] : undefined;
    const report = (rule, node, evidence) => {
      const position = source.getLineAndCharacterOfPosition(node.getStart(source));
      const finding = { path: slash(path.relative(root, filename)), line: position.line + 1, column: position.character + 1, rule, evidence, message: RULES[rule] };
      const key = JSON.stringify(finding);
      if (!seen.has(key)) { seen.add(key); findings.push(finding); }
    };
    const globalIdentifier = node => {
      if (!ts.isIdentifier(node)) return false;
      const symbol = ts.isShorthandPropertyAssignment(node.parent) ? checker.getShorthandAssignmentValueSymbol(node.parent) : checker.getSymbolAtLocation(node);
      return !symbol || !(symbol.declarations ?? []).some(declaration => !declaration.getSourceFile().isDeclarationFile);
    };
    const globalObject = (node, visited = new Set()) => {
      node = stripExpression(node);
      if (!node) return false;
      if (ts.isIdentifier(node)) {
        if (GLOBAL_OBJECTS.has(node.text) && globalIdentifier(node)) return true;
        const symbol = checker.getSymbolAtLocation(node);
        if (!symbol || visited.has(symbol)) return false;
        visited.add(symbol);
        return (symbol.declarations ?? []).some(declaration => ts.isVariableDeclaration(declaration) && declaration.initializer && (declaration.parent.flags & ts.NodeFlags.Const) && globalObject(declaration.initializer, visited));
      }
      const member = property(node);
      return !!member && GLOBAL_OBJECTS.has(member.name) && globalObject(member.object, visited);
    };
    const navigatorObject = node => {
      node = stripExpression(node);
      if (!node) return false;
      const member = property(node);
      return (ts.isIdentifier(node) && node.text === 'navigator' && globalIdentifier(node)) || (member?.name === 'navigator' && globalObject(member.object));
    };
    const capability = node => {
      node = stripExpression(node);
      if (!node) return undefined;
      if (ts.isIdentifier(node) && globalIdentifier(node)) return GLOBAL_CAPABILITIES.get(node.text);
      const member = property(node);
      if (!member) return undefined;
      if (globalObject(member.object)) {
        if (member.name === '__TAURI__' || member.name === '__TAURI_INTERNALS__') return 'FE_TAURI_GATEWAY';
        return GLOBAL_CAPABILITIES.get(member.name);
      }
      const object = stripExpression(member.object);
      if (member.name === 'credentials' && navigatorObject(object)) return 'FE_CREDENTIAL_ACCESS';
      if (member.name === 'sendBeacon' && navigatorObject(object)) return 'FE_HTTP_ACCESS';
      return undefined;
    };
    const checkImport = (specifier, node) => {
      const evidence = `import ${JSON.stringify(specifier)}`;
      for (const [rule, packages] of FORBIDDEN_PACKAGES) if (packages.some(name => packageMatches(specifier, name))) report(rule, node, evidence);
      if (NODE_IO.test(specifier)) report('FE_NODE_IO', node, evidence);
      if (specifier.startsWith('@tauri-apps/') && !TAURI_GATEWAYS.has(relative)) report('FE_TAURI_GATEWAY', node, evidence);
      if (/^(?:react-aria(?:-components)?|@react-aria\/|@react-stately\/)/.test(specifier) && !relative.startsWith('ui/primitives/')) report('FE_ARIA_WRAPPER', node, evidence);
      const resolved = resolveImport(specifier, filename, options, sourceRoot);
      if (!resolved) {
        if (isConfiguredLocal(specifier, options)) report('FE_UNRESOLVED_LOCAL_IMPORT', node, evidence);
        return;
      }
      if (resolved.includes(`${path.sep}node_modules${path.sep}`)) return;
      const targetRootRelative = slash(path.relative(root, resolved));
      if (/(?:^|\/)(?:tests?|fixtures|__tests__|__mocks__)(?:\/|$)/.test(targetRootRelative) || targetRootRelative.startsWith('docs/design/')) report('FE_PRODUCTION_TEST_IMPORT', node, evidence);
      if (!inside(resolved, sourceRoot)) { report('FE_SOURCE_ESCAPE', node, evidence); return; }
      const target = slash(path.relative(sourceRoot, resolved));
      const targetLayer = target.split('/')[0];
      const targetFeature = target.startsWith('features/') ? target.split('/')[1] : undefined;
      if (SOURCE_LAYERS.has(layer) && !SOURCE_LAYERS.has(targetLayer)) report('FE_UNCLASSIFIED_IMPORT', node, evidence);
      if (layer === 'ui' && ['app', 'bridge', 'windows', 'features'].includes(targetLayer)) report('FE_UI_BUSINESS_IMPORT', node, evidence);
      if (layer === 'bridge' && ['app', 'windows', 'features', 'ui'].includes(targetLayer)) report('FE_BRIDGE_UI_IMPORT', node, evidence);
      if (layer === 'features' && ['app', 'windows'].includes(targetLayer)) report('FE_FEATURE_UPWARD_IMPORT', node, evidence);
      if (targetFeature && feature !== targetFeature && target !== `features/${targetFeature}/index.ts`) report('FE_FEATURE_PRIVATE_IMPORT', node, evidence);
      if (targetLayer === 'bridge' && layer !== 'bridge' && !BRIDGE_PUBLIC_FILES.has(target) && !(COMPOSITION_ROOTS.has(relative) && target === 'bridge/tauri-port.ts')) report('FE_BRIDGE_PRIVATE', node, evidence);
      if (layer === 'ui' && targetLayer === 'ui') {
        const ranks = { tokens: 0, icons: 0, primitives: 1, patterns: 2 };
        const sourceRank = ranks[relative.split('/')[1]];
        const targetRank = ranks[target.split('/')[1]];
        if (sourceRank !== undefined && targetRank !== undefined && targetRank > sourceRank) report('FE_UI_LAYER_ORDER', node, evidence);
      }
    };
    if (!relative.includes('/') || !SOURCE_LAYERS.has(layer)) report('FE_SOURCE_LAYOUT', source, `source layout: ${relative}`);
    for (const diagnostic of source.parseDiagnostics) {
      const node = { getStart: () => diagnostic.start ?? 0 };
      report('FE_PARSE_ERROR', node, `TS${diagnostic.code}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, ' ')}`);
    }
    const visit = node => {
      if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) checkImport(node.moduleSpecifier.text, node.moduleSpecifier);
      if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)) {
        const value = literal(node.argument.literal);
        if (value !== undefined) checkImport(value, node.argument.literal);
      }
      if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
        const value = literal(node.moduleReference.expression);
        if (value !== undefined) checkImport(value, node.moduleReference.expression);
      }
      if (ts.isCallExpression(node)) {
        const expression = stripExpression(node.expression);
        const moduleCall = expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(expression) && expression.text === 'require' && globalIdentifier(expression));
        if (moduleCall) {
          const value = literal(node.arguments[0]);
          if (value === undefined) report('FE_DYNAMIC_MODULE', node.expression, expression.kind === ts.SyntaxKind.ImportKeyword ? 'import(non-literal)' : 'require(non-literal)');
          else checkImport(value, node.arguments[0]);
        }
        const member = property(expression);
        if (member && ['glob', 'globEager'].includes(member.name) && ts.isMetaProperty(member.object) && member.object.keywordToken === ts.SyntaxKind.ImportKeyword) report('FE_DYNAMIC_MODULE', expression, `import.meta.${member.name}`);
      }
      if (ts.isIdentifier(node)) {
        const symbol = checker.getSymbolAtLocation(node);
        const declarationName = (ts.isVariableDeclaration(node.parent) || ts.isParameter(node.parent) || ts.isFunctionDeclaration(node.parent) || ts.isImportClause(node.parent) || ts.isImportSpecifier(node.parent) || ts.isBindingElement(node.parent)) && node.parent.name === node;
        const propertyName = (ts.isPropertyAccessExpression(node.parent) && node.parent.name === node) || (ts.isPropertyAssignment(node.parent) && node.parent.name === node) || ts.isPropertySignature(node.parent) || ts.isMethodDeclaration(node.parent) || ts.isBindingElement(node.parent);
        const typeOnly = ts.isTypeReferenceNode(node.parent) || ts.isTypeQueryNode(node.parent);
        if (!declarationName && !propertyName && !typeOnly && (!symbol || globalIdentifier(node))) {
          const rule = capability(node);
          if (rule && !(rule === 'FE_TAURI_GATEWAY' && TAURI_GATEWAYS.has(relative))) report(rule, node, node.text);
        }
      }
      if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
        const rule = capability(node);
        if (rule && !(rule === 'FE_TAURI_GATEWAY' && TAURI_GATEWAYS.has(relative))) report(rule, node, node.getText(source));
      }
      if (ts.isVariableDeclaration(node) && ts.isObjectBindingPattern(node.name) && node.initializer) {
        for (const binding of node.name.elements) {
          if (binding.dotDotDotToken) continue;
          const name = binding.propertyName ?? binding.name;
          const key = ts.isIdentifier(name) || ts.isStringLiteral(name) ? name.text : undefined;
          if (globalObject(node.initializer) && GLOBAL_CAPABILITIES.has(key)) report(GLOBAL_CAPABILITIES.get(key), binding, `global destructure ${key}`);
          if (globalObject(node.initializer) && ['__TAURI__', '__TAURI_INTERNALS__'].includes(key) && !TAURI_GATEWAYS.has(relative)) report('FE_TAURI_GATEWAY', binding, `global destructure ${key}`);
          const initializer = stripExpression(node.initializer);
          if (key === 'credentials' && navigatorObject(initializer)) report('FE_CREDENTIAL_ACCESS', binding, 'navigator destructure credentials');
        }
      }
      if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken && ts.isObjectLiteralExpression(stripExpression(node.left))) {
        for (const binding of stripExpression(node.left).properties) {
          if (!ts.isPropertyAssignment(binding) && !ts.isShorthandPropertyAssignment(binding)) continue;
          const key = ts.isIdentifier(binding.name) || ts.isStringLiteral(binding.name) ? binding.name.text : undefined;
          if (globalObject(node.right) && GLOBAL_CAPABILITIES.has(key)) report(GLOBAL_CAPABILITIES.get(key), binding, `global destructure assignment ${key}`);
          if (navigatorObject(node.right) && key === 'credentials') report('FE_CREDENTIAL_ACCESS', binding, 'navigator destructure assignment credentials');
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  findings.sort((a, b) => a.path.localeCompare(b.path) || a.line - b.line || a.column - b.column || a.rule.localeCompare(b.rule));
  return { schema_version: 1, checker: 'frontend-boundaries', status: findings.length ? 'fail' : 'pass', scanned_files: files.length, rules: RULES, limitations: LIMITATIONS, findings };
}

function main() {
  const args = process.argv.slice(2);
  let root = repositoryRoot;
  let json = false;
  for (let index = 0; index < args.length; index++) {
    if (args[index] === '--json') json = true;
    else if (args[index] === '--root' && args[index + 1]) root = path.resolve(args[++index]);
    else throw new Error('Usage: node scripts/quality/frontend-boundaries.mjs [--root <repository>] [--json]');
  }
  const result = analyzeFrontend({ root });
  if (json) process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  else {
    process.stdout.write(`frontend-boundaries: ${result.status}; ${result.scanned_files} files; ${result.findings.length} findings\n`);
    for (const finding of result.findings) process.stdout.write(`${finding.path}:${finding.line}:${finding.column} ${finding.rule} ${finding.evidence}\n`);
  }
  process.exitCode = result.findings.length ? 1 : 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); }
  catch (error) {
    // Avoid leaking absolute local paths in portable CI results.
    const message = error instanceof Error ? error.message.replaceAll(repositoryRoot, '<repository>') : String(error);
    process.stderr.write(`frontend-boundaries configuration error: ${message}\n`);
    process.exitCode = 2;
  }
}
