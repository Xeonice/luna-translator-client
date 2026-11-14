/* Authored learning fixtures over paired local source text; no model/tokenizer runs. */
(function () {
  'use strict';
  const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const clone = value => JSON.parse(JSON.stringify(value));
  let requestTimer = null;
  let popover = null;
  function initialState() {
    return { unit: null, tokenIndex: null, tab: 'grammar', task: 'grammar', status: 'idle', requestId: 0, request: null, result: null, highlighted: '', error: '' };
  }
  function options(app, unit) {
    return window.LunaSettings?.readingOptions?.(app.state.settings, unit?.gameId) || { segmentation: true, furigana: true, pos: true, fontScale: 1 };
  }
  function effective(app, task, unit) {
    return window.LunaSettings?.effective?.(app.state.settings, task, unit?.gameId) || { enabled: false, connectionEnabled: false, status: 'unconfigured' };
  }
  function signature(config) {
    return JSON.stringify([config.enabled, config.connectionEnabled, config.modelId, config.revision, config.outputMode, config.historyCount]);
  }
  function sourceHTML(app, unit) {
    if (!unit) return '<span class="lp-waiting">等待游戏显示文本</span>';
    const prefs = options(app, unit);
    const tokens = unit.tokens?.length ? unit.tokens : [{ surface: unit.source }];
    const rich = prefs.segmentation !== false && unit.language !== 'zh-CN';
    let cursor = 0;
    const parts = tokens.map((token, index) => {
      const start = cursor;
      cursor += token.surface.length;
      const ref = app.state.learning?.highlighted;
      const active = app.state.learning?.unit?.id === unit.id && ref && typeof ref === 'object' && start < ref.end && cursor > ref.start;
      if (!rich || !token.meaning) {
        const plain = esc(token.surface).replace(/\n/g, '<br>');
        return active ? `<span class="lp-referenced">${plain}</span>` : plain;
      }
      const text = prefs.furigana !== false && token.reading ? `<ruby>${esc(token.surface)}<rp>（</rp><rt>${esc(token.reading)}</rt><rp>）</rp></ruby>` : esc(token.surface);
      const posClass = token.pos === '名词' ? 'noun' : token.pos === '动词' ? 'verb' : token.pos === '助词' ? 'particle' : 'other';
      return `<button class="lp-token ${prefs.pos !== false ? 'lp-pos-' + posClass : ''} ${active ? 'lp-referenced' : ''}" data-token-index="${index}" data-unit-id="${esc(unit.id)}" aria-label="查词：${esc(token.surface)}">${text}</button>`;
    }).join('');
    return `<span class="lp-source ${rich && prefs.furigana !== false ? 'lp-ruby' : ''}" lang="${unit.language === 'zh-CN' ? 'zh-CN' : 'ja'}">${parts}</span>`;
  }
  function open(app, unit, tokenIndex = null) {
    if (!unit?.source) { app.notify('还没有可供学习的文本。可先连接游戏或输入一句日语。'); return; }
    clearTimeout(requestTimer);
    const previous = app.state.learning || initialState();
    app.state.learning = { ...initialState(), unit: clone(unit), tokenIndex, tab: tokenIndex === null ? 'grammar' : 'dictionary', requestId: previous.requestId + 1 };
    app.state.windows.learning = true;
    closePopover();
    app.openWindow('learning');
  }
  function close(app) {
    const state = app.state.learning;
    if (!state) return;
    clearTimeout(requestTimer);
    state.requestId += 1;
    if (state.status === 'loading') { state.status = 'cancelled'; state.error = '已停止这次分析，原句保留。'; }
    closePopover();
  }
  function gameTitle(unit) {
    return window.LunaDesignData.games.find(game => game.id === unit?.gameId)?.subtitle || '手动文本';
  }
  function dictionaryHTML(app, state) {
    const unit = state.unit;
    const token = unit.tokens?.[state.tokenIndex ?? -1];
    if (!token?.meaning) return '<div class="lp-empty"><span class="lp-empty-symbol">' + window.LunaIcons.svg('book') + '</span><h3>从原文中选择一个词</h3><p>内置词典在本机工作，不需要先配置模型。</p></div>';
    return `<article class="lp-dictionary"><div class="lp-dict-origin">内置词典</div><h2 lang="ja">${esc(token.surface)}</h2><p class="lp-dict-reading">${esc(token.reading || token.surface)}<span>${esc(token.pos)}</span></p><dl class="lp-word-facts"><div><dt>词面</dt><dd>${esc(token.surface)}</dd></div><div><dt>词元</dt><dd>${esc(token.lemma || token.surface)}</dd></div></dl><h3>释义</h3><p class="lp-definition">${esc(token.meaning)}</p><div class="lp-source-example"><small>词语所属原句</small><p lang="ja">${esc(unit.source)}</p></div><div class="lp-dictionary-actions"><button data-learn-action="word-explain">结合本句解释…</button><button data-learn-action="grammar-tab">分析本句文法…</button></div><p class="lp-subtle">本地释义与模型解释分别呈现；模型结果不会改写辞书内容。</p></article>`;
  }
  function explain(unit) {
    const text = unit.source;
    if (text.includes('その風景')) return {
      parts: [{ text: 'その風景には', role: '范围与主题' }, { text: '悲しさも、痛みも', role: '并列对象' }, { text: 'なかった', role: '否定过去' }],
      points: [{ title: 'には · 范围和主题', quote: 'その風景には', body: '「に」提示景象中的范围，「は」将这一范围作为话题。可自然理解为“那片景色中”。' }, { title: 'も…も…ない · 两者都没有', quote: '悲しさも、痛みも', body: '两个「も」与否定谓语呼应，表示悲伤和痛楚两者都不存在。不是“也有悲伤、也有痛楚”。' }, { title: 'なかった · 过去的否定', quote: 'なかった', body: '「ない」变为过去形式「なかった」，对应叙述当时“没有”的状态。' }],
      uncertainty: '「悲しさ」「痛み」可指情感或感受，具体对象需结合前后文。原句没有明说人物的心理原因。'
    };
    if (text.includes('泣かなかった')) return {
      parts: [{ text: 'だから', role: '承接理由' }, { text: '俺は', role: '主题' }, { text: '泣かなかった', role: '否定过去' }, { text: 'のだろうか', role: '自问' }],
      points: [{ title: '泣かなかった · 没有哭', quote: '泣かなかった', body: '「泣く」的否定过去形式，描述当时未发生的动作。' }, { title: 'のだろうか · 带推测的自问', quote: 'のだろうか', body: '说话者在探究原因或确认自己的理解，不是在直接向对方断言结论。' }], uncertainty: '「だから」承接的具体原因来自前文，仅凭本句无法确认。'
    };
    if (text.includes('ひしひしと')) return {
      parts: [{ text: '秋を', role: '感受到的对象' }, { text: 'ひしひしと感じる', role: '修饰季节' }, { text: '季節になっていた', role: '状态变化' }],
      points: [{ title: 'ひしひしと · 强烈而深切地', quote: 'ひしひしと', body: '副词修饰「感じる」，强调逐渐切实感受到秋意。' }, { title: '～になっていた · 已经进入某种状态', quote: '季節になっていた', body: '「になる」表示变化，「ていた」回顾变化之后的状态。' }], uncertainty: '叙述者何时意识到季节变化，需要结合前面的描写理解。'
    };
    if (text.includes('何ら変わりない')) return {
      parts: [{ text: 'いつもと', role: '比较基准' }, { text: '何ら変わりない', role: '修饰生活' }, { text: '生活', role: '中心名词' }],
      points: [{ title: '何ら～ない · 丝毫没有', quote: '何ら変わりない', body: '「何ら」与否定表达呼应，强化“没有任何变化”。' }, { title: '名词句的省略', quote: '生活', body: '原句以名词收束，像叙述中的片段描写。中文可自然译为“一如既往的生活”。' }], uncertainty: '原句并未直接表达对生活满意或不满，情绪需要上下文支持。'
    };
    if (text.includes('起こされ')) return {
      parts: [{ text: '隣りの幼なじみに', role: '被动行为的施事' }, { text: '起こされ', role: '前一动作' }, { text: 'けだるい体で家を出る', role: '后续动作' }],
      points: [{ title: '起こされ · 被叫醒', quote: '起こされ', body: '「起こす」的被动连用形式，后面接继续叙述的动作；「に」引出叫醒自己的人。' }, { title: 'で · 以某种状态', quote: 'けだるい体で', body: '这里表示离家时身体的状态，可译为“拖着倦怠的身体”。' }], uncertainty: '原句省略主语；“我”是依叙事视角补出的自然译法。'
    };
    if (text.includes('それはオレの言葉')) return {
      parts: [{ text: 'それは', role: '主题' }, { text: 'オレの言葉', role: '判断对象' }, { text: 'だったのだろうか', role: '过去判断与自问' }],
      points: [{ title: 'だったのだろうか · 对过去的自问', quote: 'だったのだろうか', body: '「だった」回顾过去的判断，「のだろうか」为这一判断加上推测和疑问。' }, ...(text.includes('あるいは') ? [{ title: 'あるいは · 另一种可能', quote: 'あるいは', body: '提出另一种解释。后面的「誰かの」省略了前文可恢复的名词「言葉」。' }] : [])], uncertainty: '「それ」指向的内容来自上文；不能根据这一句补写未出现的剧情。'
    };
    if (text.includes('何も無かった')) return {
      parts: [{ text: 'ここには', role: '范围与主题' }, { text: '何も', role: '全面否定' }, { text: '無かった', role: '过去状态' }],
      points: [{ title: '何も～ない · 什么也没有', quote: '何も無かった', body: '疑问词与「も」加否定表达全面否定。' }, ...(text.includes('音も') ? [{ title: 'も…も… · 并列强调', quote: '音も無く、色も無く、風も無く', body: '通过重复「も」与否定的连用形式，逐项强调声音、颜色和风都不存在。' }] : [])], uncertainty: '这是一种叙述性描写；不能仅凭否定表达判断它是现实、梦境或比喻。'
    };
    if (text.includes('図書館')) return {
      parts: [{ text: '放課後', role: '时间' }, { text: '図書館で', role: '场所' }, { text: '彼女と', role: '共同对象' }, { text: '待ち合わせをした', role: '谓语' }],
      points: [{ title: 'で · 动作发生的场所', quote: '図書館で', body: '「で」标记会面发生的地点，这里是图书馆。' }, { title: 'と · 共同对象', quote: '彼女と', body: '表示与她一起进行会面行为。' }, { title: '名词＋をする', quote: '待ち合わせをした', body: '「待ち合わせ」接「をする」，过去式「した」表达已经发生的会面安排。' }], uncertainty: '原句省略了主语；中文中的“我”是结合语境的补充。'
    };
    const known = unit.tokens?.filter(token => token.meaning && token.surface.length > 1).slice(0, 3) || [];
    return { parts: known.map(token => ({ text: token.surface, role: token.pos })), points: known.map(token => ({ title: token.surface + ' · 本句词语', quote: token.surface, body: token.meaning })), uncertainty: '单个词义不能代替整句判断。省略、指代与叙述语气仍须结合允许的上下文核对。' };
  }
  function resultHTML(state) {
    const result = state.result;
    if (!result) return '';
    if (state.task === 'word') {
      const token = state.unit.tokens[state.tokenIndex];
      return `<div class="lp-answer"><h3>${esc(token?.surface || '所选词语')} · 结合本句理解</h3><p>${esc(token?.meaning || '结合原句核对这个表达的含义。')}</p><p>词语解释以这条已固定原句为范围；本地辞书的词元和读音仍保留原始来源。</p></div>`;
    }
    if (state.unit.language === 'zh-CN') return '<div class="lp-answer"><h3>中文原文的阅读说明</h3><p>此安装版本输出中文原文，不生成日语分词或假名。人物称呼和语气应结合当前对话判断；模型解释不改写已经采集的原文。</p></div>';
    const data = result.data;
    if (result.outputMode === 'text') return `<div class="lp-answer"><div class="lp-mode-label">文字解释</div>${data.points.map(point => `<h3>${esc(point.title)}</h3><p>${esc(point.body)}</p>`).join('')}<p class="lp-subtle">${esc(data.uncertainty)}</p></div>`;
    return `<div class="lp-answer"><h3>句子结构</h3><div class="lp-clauses">${data.parts.filter(part => state.unit.source.includes(part.text)).map(part => `<button data-learn-action="reference" data-reference="${esc(part.text)}"><strong lang="ja">${esc(part.text)}</strong><small>${esc(part.role)}</small></button>`).join('')}</div>${data.points.filter(point => state.unit.source.includes(point.quote)).map(point => `<section class="lp-analysis-point"><h3>${esc(point.title)}</h3><p><button class="lp-inline-reference" data-learn-action="reference" data-reference="${esc(point.quote)}" lang="ja">${esc(point.quote)}</button>：${esc(point.body)}</p></section>`).join('')}<details class="lp-uncertainty"><summary>可能存在的歧义</summary><p>${esc(data.uncertainty)}</p></details></div>`;
  }
  function render(app) {
    const state = app.state.learning;
    const unit = state.unit;
    queueMicrotask(() => updatePopover(app));
    if (!unit) return '<div class="lp-empty"><h3>从当前句或阅读记录开始</h3><p>在字幕“更多”中选择“分析本句文法”，或点击词语。</p></div>';
    const config = effective(app, state.task, unit);
    if (state.status === 'loading' && signature(config) !== state.request?.signature) {
      clearTimeout(requestTimer); state.requestId += 1; state.status = 'cancelled'; state.error = '模型配置已改变，原请求已停止。请确认新配置后重新开始。';
    }
    const latest = app.state.current && (app.state.current.id !== unit.id || app.state.current.revision !== unit.revision);
    const configured = config.enabled && config.connectionEnabled;
    const unavailable = !config.connectionEnabled && config.modelId;
    const recordIndex = app.state.history.findIndex(record => record.id === unit.id);
    const recordLabel = recordIndex < 0 ? '已固定原句' : '已固定原句 · 阅读记录 #' + String(recordIndex + 1).padStart(3, '0');
    let body = `<div class="lp-toolbar"><div class="lp-segments" role="tablist" aria-label="学习内容"><button data-learn-action="grammar-tab" role="tab" aria-selected="${state.tab === 'grammar'}">本句精读</button><button data-learn-action="dictionary-tab" role="tab" aria-selected="${state.tab === 'dictionary'}">内置词典</button></div><button class="lp-text-button" data-learn-action="settings">模型设置…</button></div><div class="lp-scroll"><div class="lp-document-meta"><span>${esc(gameTitle(unit))}</span><span>${esc(recordLabel)}</span></div><div class="lp-original" data-unit-id="${esc(unit.id)}">${sourceHTML(app, unit)}</div>${unit.translation ? `<p class="lp-document-translation">${esc(unit.translation)}</p>` : ''}`;
    if (latest) body += '<div class="lp-latest-note"><span>字幕已到下一句，此处仍保留原句。</span><button data-learn-action="latest">分析最新一句</button></div>';
    if (unit.language === 'zh-CN') body += '<div class="lp-language-note">中文原文 · 不应用日语注音；内置日语词典可在手动文本中使用。</div>';
    if (state.tab === 'dictionary') body += dictionaryHTML(app, state);
    else if (!configured) body += `<div class="lp-setup"><span class="lp-setup-icon">${window.LunaIcons.svg(unavailable ? 'pause' : 'book')}</span><h3>${unavailable ? '服务连接已停用' : '为这类理解选择一个模型'}</h3><p>${unavailable ? '原句、词语和用途绑定都已保留。恢复连接后，再由你开始。' : `已保留当前原句。${state.task === 'word' ? '语境查词' : '句子精读'}与实时翻译分别配置，不会自动借用翻译模型。`}</p><button class="primary" data-learn-action="settings">${unavailable ? '查看连接与用途…' : '配置' + (state.task === 'word' ? '语境查词' : '句子精读') + '…'}</button><small>原文与内置词典继续可用</small></div>`;
    else if (state.status === 'loading') body += '<div class="lp-progress" role="status"><span class="lp-spinner"></span><h3>正在理解这句原文</h3><p>分析对象已固定，你可以继续阅读游戏。</p><button data-learn-action="cancel">取消分析</button></div>';
    else if (state.status === 'failure' || state.status === 'cancelled') body += `<div class="lp-failure" role="status"><h3>${state.status === 'failure' ? '这次分析未完成' : '分析已停止'}</h3><p>${esc(state.error)}</p><div><button data-learn-action="start">重新分析本句</button><button data-learn-action="settings">修改模型设置…</button></div></div>`;
    else if (state.status === 'complete') body += resultHTML(state);
    else body += `<div class="lp-ready"><h3>${state.task === 'word' ? '结合本句理解所选词语' : '看懂句子结构，而不打断阅读'}</h3><p>将发送这条原句${config.historyCount ? '及最多 ' + Number(config.historyCount) + ' 条允许的近期上下文' : '，不附加阅读历史'}。解释由你主动开始。</p><button class="primary" data-learn-action="start">${state.task === 'word' ? '解释这个词' : '开始分析本句'}</button></div>`;
    const provenance = state.result?.provider || config.modelName || '尚未选择模型';
    body += `</div><footer class="lp-footer"><div><strong>${esc(provenance)}</strong><small>${state.result ? '本条结果的模型来源' : (config.source || '用途独立配置')} · ${state.result?.outputMode === 'text' ? '文字解释' : '原句与解释分别保留'}</small></div>${state.status === 'complete' ? '<button data-learn-action="start">重新分析</button>' : '<button data-learn-action="return-reading">继续阅读</button>'}</footer>`;
    return body;
  }
  function begin(app) {
    const state = app.state.learning;
    if (!state.unit) return;
    const config = effective(app, state.task, state.unit);
    if (!config.enabled || !config.connectionEnabled) { app.openSettings(state.task); return; }
    clearTimeout(requestTimer);
    const identity = ++state.requestId;
    state.status = 'loading'; state.result = null; state.error = '';
    state.request = { signature: signature(config), unitId: state.unit.id, provider: config.modelName, revision: config.revision };
    app.render();
    requestTimer = setTimeout(() => {
      if (state !== app.state.learning || state.requestId !== identity) return;
      if (signature(effective(app, state.task, state.unit)) !== state.request.signature) { state.status = 'cancelled'; state.error = '配置已改变，原请求已停止。'; app.render(); return; }
      if (app.state.fault === 'analysis') { state.status = 'failure'; state.error = '解释服务暂时没有响应。原句已保留；翻译和基础查词继续可用。'; }
      else {
        state.status = 'complete'; state.result = { unitId: state.unit.id, provider: config.modelName, revision: config.revision, outputMode: config.outputMode || 'structured', data: explain(state.unit) };
        const record = app.state.history.find(row => row.id === state.unit.id);
        if (record) record.analysis = clone(state.result);
      }
      app.render();
    }, 1000);
  }
  function closePopover() {
    const anchor = popover?.anchor;
    popover = null;
    const node = document.getElementById('word-popover');
    if (node) node.hidden = true;
    if (anchor?.isConnected) anchor.focus({ preventScroll: true });
  }
  function updatePopover(app) {
    if (!popover) return;
    let node = document.getElementById('word-popover');
    if (!node) { node = document.createElement('aside'); node.id = 'word-popover'; document.body.appendChild(node); }
    const { unit, index, rect } = popover;
    const token = unit.tokens[index];
    node.hidden = false; node.className = 'lp-word-popover'; node.setAttribute('role', 'dialog'); node.setAttribute('aria-label', '词语：' + token.surface);
    const old = app.state.current?.id !== unit.id;
    node.innerHTML = `<div class="lp-popover-top"><span>内置词典${old ? ' · 原句已固定' : ''}</span><button data-learn-action="close-popover" aria-label="关闭词卡">${window.LunaIcons.svg('x')}</button></div><h2 lang="ja">${esc(token.surface)}</h2><p class="lp-popover-reading">${esc(token.reading || token.surface)} <span>${esc(token.pos)}</span></p><p class="lp-popover-definition">${esc(token.meaning)}</p>${token.lemma !== token.surface ? `<small>词元：${esc(token.lemma)}</small>` : ''}<div class="lp-popover-actions"><button data-learn-action="full-dictionary">完整词典与学习 ${window.LunaIcons.svg('external-link')}</button><button data-learn-action="popover-grammar">分析本句文法…</button><button data-learn-action="popover-word">结合本句解释…</button></div>`;
    const width = Math.min(300, innerWidth - 30);
    node.style.width = width + 'px'; node.style.left = Math.max(15, Math.min(rect.left, innerWidth - width - 15)) + 'px';
    const top = rect.top > node.offsetHeight + 85 ? rect.top - node.offsetHeight - 12 : Math.min(rect.bottom + 12, innerHeight - node.offsetHeight - 15);
    node.style.top = Math.max(65, top) + 'px';
  }
  function resolveUnit(app, id) {
    if (app.state.learning?.unit?.id === id) return app.state.learning.unit;
    if (app.state.current?.id === id) return app.state.current;
    return app.state.history.find(row => row.id === id);
  }
  document.addEventListener('click', event => {
    const app = window.LunaDesign;
    if (!app) return;
    const tokenButton = event.target.closest('[data-token-index]');
    if (tokenButton) {
      const unit = resolveUnit(app, tokenButton.dataset.unitId);
      const index = Number(tokenButton.dataset.tokenIndex);
      if (!unit?.tokens?.[index]?.meaning) return;
      popover = { unit: clone(unit), index, rect: tokenButton.getBoundingClientRect().toJSON(), anchor: tokenButton };
      updatePopover(app); return;
    }
    const action = event.target.closest('[data-learn-action]');
    if (action) {
      const state = app.state.learning;
      switch (action.dataset.learnAction) {
        case 'close-popover': closePopover(); break;
        case 'full-dictionary': if (popover) open(app, popover.unit, popover.index); break;
        case 'popover-grammar': if (popover) open(app, popover.unit); break;
        case 'popover-word': if (popover) { const chosen = popover; open(app, chosen.unit, chosen.index); app.state.learning.task = 'word'; app.state.learning.tab = 'grammar'; app.render(); } break;
        case 'grammar-tab': state.tab = 'grammar'; state.task = 'grammar'; app.render(); break;
        case 'dictionary-tab': state.tab = 'dictionary'; app.render(); break;
        case 'word-explain': state.task = 'word'; state.tab = 'grammar'; state.status = 'idle'; state.result = null; app.render(); break;
        case 'settings': app.openSettings(state.task); break;
        case 'start': begin(app); break;
        case 'cancel': clearTimeout(requestTimer); state.requestId += 1; state.status = 'cancelled'; state.error = '已取消这次请求，原句保留。服务仍可能产生用量。'; app.render(); break;
        case 'latest': if (app.state.current) open(app, app.state.current); break;
        case 'return-reading': app.closeWindow('learning'); break;
        case 'reference': {
          const text = action.dataset.reference;
          const start = state.unit.source.indexOf(text);
          state.highlighted = start < 0 ? '' : { start, end: start + text.length, text };
          app.render(); break;
        }
      }
      return;
    }
    // Review controls emulate external game updates rather than a click in the product.
    if (popover && !event.target.closest('#word-popover, .review-bar')) closePopover();
  });
  document.addEventListener('keydown', event => { if (event.key === 'Escape' && popover) { event.preventDefault(); closePopover(); } });
  function annotate(source) {
    if (source === window.LunaDesignData.manualSample.source) return clone(window.LunaDesignData.manualSample.tokens);
    const dictionary = [...window.LunaDesignData.lexicon].sort((a, b) => b.surface.length - a.surface.length);
    const tokens = [];
    let position = 0, gap = '';
    while (position < source.length) {
      const found = dictionary.find(entry => source.startsWith(entry.surface, position));
      if (!found) { gap += source[position++]; continue; }
      if (gap) { tokens.push({ surface: gap }); gap = ''; }
      tokens.push(clone(found)); position += found.surface.length;
    }
    if (gap) tokens.push({ surface: gap });
    return tokens;
  }
  window.LunaLearning = { initialState, render, sourceHTML, open, close, begin, closePopover, annotate };
})();
