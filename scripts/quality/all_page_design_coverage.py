#!/usr/bin/env python3
"""Complete *current discovered* UI inventory; metadata is never product evidence.

This code owns obligations. The JSON is a reviewable snapshot, not a mechanism to
install captures, grant reference approval, or erase missing design states.
Production capture/execution is deliberately absent. Re-run AST discovery on each
check; new pages/overlays/exported shared components must be explicitly mapped.
"""
from __future__ import annotations
import argparse
import csv
import hashlib
import json
from pathlib import Path
import re
import subprocess
import design_authority as authority

ROOT = Path(__file__).resolve().parents[2]
CONTRACT = 'docs/development/all-page-design-coverage.json'
DOCUMENT = 'docs/development/all-page-design-coverage.md'
TARGET = 'guided-service-reading-20261030'
DESIGN = 'docs/design/first-product-v1'
ENTRY = DESIGN + '/candidates/' + TARGET + '/index.html'
HISTORY_ENTRY = DESIGN + '/candidates/history-session-paging-20261031/index.html'
PREFIX = 'apps/desktop/src/'
LAYERS = ['real_react', 'packaged_native']
COMMON_STATES = ('normal', 'empty', 'loading', 'error', 'disabled', 'long_text', 'light', 'narrow', 'zoom_200', 'keyboard_focus_escape')
STATE_LABELS = {'normal':'正常内容', 'empty':'空内容', 'loading':'正在加载或处理', 'error':'错误与恢复', 'disabled':'不可用及原因', 'long_text':'长文字与长路径', 'light':'浅色主题', 'narrow':'窄窗口', 'zoom_200':'200%文字缩放', 'keyboard_focus_escape':'键盘、焦点与取消'}
# Code-owned quality responsibilities outside the quality directory. This is
# the fixed independent PNG recomputation command invoked by product_execution;
# adjacent contract/build scripts have no implied nonproduction exemption.
QUALITY_ONLY_PATHS = frozenset({'scripts/contracts/compare_product_capture.mjs'})

def click(selector): return {'action':'click', 'selector':selector}
def fill(selector, value): return {'action':'fill', 'selector':selector, 'value':value}
def select(selector, value): return {'action':'select_option', 'selector':selector, 'value':value}
def evaluate(script): return {'action':'evaluate_design_api', 'script':script, 'scope':'page_local_design_simulation','execution_mode':'invoke_function_body'}
def visible(selector): return {'action':'assert_visible', 'selector':selector}
def state(script): return evaluate('const app=window.LunaDesign;'+script+';app.render();')

def spec(title, cases, components, paths, scenario, selector, steps=(), present=None, extras=(), needs_mapping=(), static=False, gap=''):
    return {'title':title, 'cases':cases.split(), 'components':components.split(),
            'paths':[PREFIX+p for p in paths.split()], 'scenario':scenario,
            'selector':selector, 'steps':list(steps), 'present':present or {},
            'extras':list(extras), 'needs_mapping':list(needs_mapping), 'static':static, 'gap':gap}

def main_steps(page): return [click('[data-action="open-main"]'), click('[data-page="'+page+'"]')]
def settings_steps(category): return [click('[data-settings-action="category"][data-value="'+category+'"]')]

# Each special state remains an independent obligation. Missing states keep a
# replayable base route and a blocked expectation, never a fabricated screenshot.
SPECS = {
'main:library':spec('游戏库','FP-01 FP-02 GL-01 GL-07 GL-09','C09 C11','features/library/LibraryScreen.tsx','existing-library','#main-window',present={'normal':[], 'empty':[{'action':'navigate_scenario','scenario':'first-run'}]},extras=['search_no_match','cover_missing','selected_detail'],gap='生产/设计游戏记录、封面及窗口关联需当前参照复核。'),
'main:session':spec('当前会话与三个独立数据集','FP-03 FP-04 FP-05 FP-09 RW-17 RW-19 RW-20 RW-21 RW-22 RW-23','C16 C17 C18 C35 C36','features/reading/ReadingPanel.tsx features/reading/SessionRecordPanel.tsx features/capture/WindowPreview.tsx','window-preview','#main-window',present={'normal':[], 'error':[select('#review-fault','integrity')]},extras=['body_collapsed','body_expanded','extraction_collapsed','extraction_expanded','hook_collapsed','hook_expanded','query_no_match','pagination','paused_follow_new_count','export_review','translation_pending','translation_failed','translation_cancelled','translation_disabled','fixed_old_record','comparison_independent_failure']),
'main:history':spec('历史记录与人工修正','FP-09 MC-20','C23 C24 C20','features/history/HistoryScreen.tsx','history','#main-window',present={'normal':[], 'empty':[{'action':'navigate_scenario','scenario':'first-run'},*main_steps('history')], 'error':[select('#review-fault','save'),click('[data-action="edit-history"]'),click('[data-action="save-history-edit"]')]},extras=['game_filter','session_filter','pagination','detail_loading','detail_error','locate_current','correction_edit','correction_conflict','retranslate','frozen_learning'],needs_mapping=['session_filter','detail_loading','detail_error','correction_conflict'],gap='生产含会话筛选/游标分页与详情错误，当前HTML历史页没有等价完整状态。'),
'main:manual':spec('手动翻译','FP-10 FP-06 FP-07 FP-08','C25 C03 C16 C17','features/manual-input/ManualInputScreen.tsx','reading','#main-window',main_steps('manual'),present={'normal':[], 'empty':[], 'loading':[state("app.state.manualStatus='pending'")], 'error':[state("app.state.manualStatus='failed';app.state.manualResult=''")]},extras=['cancelled_input_retained','completed_history','unconfigured','source_language','clear_input'],needs_mapping=['source_language'],gap='生产源语言选择与设计固定日中提示不同；不得默认为一致。'),
'main:diagnostics':spec('主窗诊断与恢复','FP-12 RW-18 RW-23','C33 C08','features/diagnostics/DiagnosticsScreen.tsx','failure','#main-window',present={'normal':[], 'error':[]},extras=['no_events','redacted_details','integrity_recovery_confirm','game_exited','long_native_error']),
'window:main':spec('主窗壳与启动恢复','FP-01 FP-05 FP-11 FP-12','C34 C09 C08','windows/main/MainWindow.tsx app/ErrorBoundary.tsx ui/patterns/WindowFrame.tsx','existing-library','#main-window',present={'normal':[]},extras=['snapshot_retry','error_boundary','close_preserves_session','dock_restore','quit_confirm','system_menu','navigation_error'],needs_mapping=['snapshot_retry','error_boundary','navigation_error']),
'window:hud':spec('阅读浮窗','FP-04 FP-05 FP-07 RW-07 RW-08 RW-24 RW-25 RW-26 RW-27 RW-28 RW-29 RW-30 RW-32','C16 C17 C18 C19 C34','windows/hud/HudWindow.tsx features/language/SourceText.tsx','reading','#hud-window',present={'normal':[], 'empty':[{'action':'navigate_scenario','scenario':'first-run'}], 'error':[select('#review-fault','translation')], 'disabled':[state("app.state.session.translationEnabled=false;app.state.current.translationStatus='disabled'")]},extras=['plain','ruby_only','ruby_pos','unknown_reading','lexical_error','compact_500','comfortable_560','more_menu','service_selector','reduced_transparency','drag_restore','hide_restore','game_exit','streaming','cancelled','native_shortcut_failure']),
'window:learning':spec('独立学习与内置词典','FP-07 FP-08 MC-06 MC-17 MC-22','C19 C20 C21 C22 C34','windows/learning/LearningWindow.tsx features/study/StudyScreen.tsx','learning','#learning-window',present={'normal':[], 'empty':[state('app.state.learning.unit=null')], 'disabled':[]},extras=['dictionary_tab','dictionary_loading','dictionary_error','dictionary_no_entries','dictionary_multiple_entries','unconfigured_grammar','disabled_connection','ready','queued','running','streaming','cancelled','invalidated','failed','complete_structured','complete_text','reference_highlight','old_sentence_fixed','non_japanese','return_from_configuration'],needs_mapping=['queued','streaming','dictionary_loading','dictionary_error','dictionary_multiple_entries'],gap='StudyScreen具有队列/流式与查询错误，旧learning脚本未实现全部对应渲染。'),
'window:settings':spec('设置窗口壳、搜索及返回','FP-06 FP-11 MC-06','C10 C34','windows/settings/SettingsWindow.tsx','settings','#settings-window',present={'normal':[], 'empty':[fill('[data-settings-field="search"]','不存在的设置分类')]},extras=['search_match','return_main_focus','unsaved_navigation_confirm','snapshot_retry'],needs_mapping=['snapshot_retry']),
'surface:word-card':spec('原生临时词卡独立surface','FP-07 FP-08 RW-26 RW-29 RW-30','C19 C20 C22','features/language/WordCardSurface.tsx features/language/WordCard.tsx features/language/SourceText.tsx','reading','#word-popover',[click('#hud-source [data-token-index="0"]')],extras=['frozen_word','multiple_entries','unknown_reading','no_entry','native_content_fit','escape_close','open_learning','context_word'],needs_mapping=['normal','empty','loading','error','disabled','long_text','light','narrow','zoom_200','keyboard_focus_escape','frozen_word','multiple_entries','unknown_reading','no_entry','native_content_fit','escape_close','open_learning','context_word'],gap='HTML的#word-popover不是生产?surface=word-card原生surface等价参照；需独立明确布局/边界及实际内容撑窗状态。'),
}

SETTINGS = {
'general':('通用','FP-11 RW-28','C32', 'general',['scope_global','scope_game','restore_inheritance','save_failed'],False,'设计通用阅读行为字段与生产偏好投影需逐项对照。'),
'capture':('输入与HOOK','FP-03 GL-14 GL-19','C15','input',['connected','disconnected','return_target_selection','dedupe','merge_wait'],False,'设计含记住来源、去重/合并字段；生产该页目前为摘要和返回入口。'),
'ocr':('OCR范围说明','FP-10','C08','ocr',[],True,'首版排除OCR；只验证说明、无执行入口，不新增加载或运行能力。'),
'processing':('文本处理','FP-03 FP-09 RW-20','C15','processing',['dedupe','merge_wait','original_preserved'],False,'设计可编辑去重/合并，生产当前为说明；差距未关闭。'),
'speech':('语音范围说明','FP-11','C08','speech',[],True,'首版排除语音；只验证说明且无执行入口。'),
'display':('显示与快捷键','FP-11 RW-08 RW-10 RW-11 RW-25 RW-30','C32 C18','display',['comfortable','compact','font_scale','reduced_material','native_material_failure','shortcut_draft','shortcut_active','shortcut_conflict','shortcut_registration_failed','restore_position','scope_game'],False,'原生材质/系统减少透明度/真实快捷键不由网页模拟证明。'),
'network':('网络范围与连接入口','FP-06 MC-14','C26','network',['open_connection_preserves_context'],True,'仅连接网络策略及后续范围说明；实际连接在翻译服务维护。'),
'data':('配置与数据','FP-11 MC-01 MC-16 MC-21','C31 C32','data',['safe_export','import_preflight','import_conflict','cancel_import','apply_import_draft','restore_saved','resource_loading','resource_missing','resource_error','profile_switch'],False,'设计含导入预检/安全导出/恢复草稿；生产此分类仅配置摘要与资源检查，不能宣称对齐。'),
'diagnostics':('设置诊断','FP-12 MC-03','C33','diagnostics',['independent_purpose_status','redacted_diagnostic','generate_failed'],False,'设置中的配置诊断与生产复用主窗DiagnosticsScreen的布局/字段不同。'),
 'translation':('引导式翻译服务配置','FP-06 MC-01 MC-02 MC-03 MC-04 MC-05 MC-07 MC-08 MC-09 MC-10 MC-11 MC-12 MC-13 MC-14 MC-15 MC-19 MC-22','C26 C27 C28 C29 C30 C31','translation',['purpose_brand_interface','required_fields','conditional_fields','custom_interface','masked_credentials','typed_credentials','test_running','test_failed','test_cancelled','test_stale','known_auth_failure_retained','save_no_select','delete_confirm','copy_confirm','learning_bindings','advanced_network','offline_unknown_resource','preset_blocker','global_game_scope','restore_inheritance'],False,'生产当前仍为用途/连接/模型/参数/运行/术语分页，不是guided服务编辑目录；布局与公共行为差距保留。'),
}
for key,(title,cases,components,category,extras,static,gap) in SETTINGS.items():
    paths='windows/settings/SettingsWindow.tsx'
    if key=='translation': paths+=' features/model-config/ModelConfigScreen.tsx'
    if key in ('general','display','data'): paths+=' features/preferences/PreferencesScreen.tsx'
    if key=='diagnostics': paths+=' features/diagnostics/DiagnosticsScreen.tsx'
    SPECS['settings:'+key]=spec(title,cases,components,paths,'settings','#settings-window',settings_steps(category),present={'normal':[]},extras=extras,static=static,gap=gap)

OVERLAYS = [
('CaptureSheet:e1434a9e7d','运行目标与正文来源','GL-11 GL-12 GL-13 GL-14 GL-15 GL-17 GL-19 GL-20','C05 C13 C14 C15 C35','features/capture/CaptureSheet.tsx','existing-library',[click('#library-running')],'#app-dialog',['window_list','process_list','selected_target','self_disabled','unknown_target','switch_confirm','attach_in_progress','attach_failed','waiting_candidates','streams_unselected','streams_selected','reselect_streams']),
('LibraryScreen:f3c016c929','添加／编辑／批量核对游戏','GL-02 GL-03 GL-04 GL-05 GL-06 GL-07 GL-08 GL-09 GL-19','C05 C12','features/library/LibraryScreen.tsx','first-run',[click('#library-add')],'#app-dialog',['single_file','batch_review','batch_next','edit_existing','invalid_metadata','save_failed','duplicate_reused','cancel_preserves_library']),
('LibraryScreen:828e15dd12','移出游戏库确认','GL-10 GL-19','C05 C11','features/library/LibraryScreen.tsx','existing-library',[click('[data-action="game-remove"]')],'#app-dialog',['cancel','confirm_preserves_session','archive_failed']),
('HudWindow:6f81579d6a','HUD更多阅读菜单','FP-05 RW-32','C06 C18','windows/hud/HudWindow.tsx','reading',[click('#hud-more')],'#hud-menu',['keyboard_open','escape_restore_focus','disconnect_confirm','unbind_preserves_capture','reset_hud']),
('ModelConfigScreen:92d4a07590','配置应用确认','MC-12','C05 C31','features/model-config/ModelConfigScreen.tsx','settings',[],'#app-dialog',['summary_scope','apply_failed','stale_revision','cancel']),
('ModelConfigScreen:d6953cce06','模型复制确认','MC-08','C05 C31','features/model-config/ModelConfigScreen.tsx','settings',[],'#app-dialog',['shared_connection_warning','copy_name','stale_revision','copy_failed','cancel']),
('ConfigurationLifecycleActions:8626d9f075','删除连接或模型确认','MC-13 MC-19','C05 C26 C31','features/model-config/ConfigurationLifecycleActions.tsx','settings',[],'#app-dialog',['connection_references','model_references','inflight_invalidated','delete_failed','cancel']),
('ConfigurationLifecycleActions:9fb87066f2','连接复制确认','MC-08','C05 C26','features/model-config/ConfigurationLifecycleActions.tsx','settings',[],'#app-dialog',['credential_scope','copy_failed','cancel']),
('CredentialRevocationPanel:7f58db8661','系统凭据撤销确认','MC-04 MC-11 MC-13','C05 C30','features/model-config/CredentialRevocationPanel.tsx','settings',[],'#app-dialog',['dependent_objects','inflight_invalidated','revoke_failed','cancel']),
('RuntimeConfigurationPanel:3cfa833a01','恢复已应用连接确认','MC-19','C05 C31','features/model-config/RuntimeConfigurationPanel.tsx','settings',[],'#app-dialog',['frozen_applied_version','restore_failed','cancel']),
('RuntimeConfigurationPanel:4812c5b249','恢复已应用用途确认','MC-12 MC-19','C05 C31','features/model-config/RuntimeConfigurationPanel.tsx','settings',[],'#app-dialog',['scope_binding','restore_failed','cancel']),
('SessionRecordPanel:208683c199','会话导出确认','FP-09 RW-22','C05 C36','features/reading/SessionRecordPanel.tsx','window-preview',[],'#app-dialog',['body_export','extraction_export','hook_export','filtered_export','redacted_export','export_error','cancel']),
]
for key,title,cases,components,paths,scenario,steps,selector,extras in OVERLAYS:
    # Only reviewed actual navigation above is declared available. Production-only
    # confirmations without a verified replay stay needs_mapping; absence is not inferred.
    present={'normal':[]} if steps else {}
    needs_mapping=[] if steps else list(COMMON_STATES)+extras
    SPECS['overlay:'+key]=spec(title,cases,components,paths,scenario,selector,steps,present,extras,needs_mapping,gap='生产弹层与当前设计的对象/版本/引用/布局需要独立参照；未可重放状态保持blocked。')


# A real C35 detail Sheet was added by the compact-preview implementation.
# It is an independently inventoried surface, not an exemption under its card.
preview_details = spec(
    '预览详情与只读诊断', 'FP-02 FP-03 FP-05 FP-11 FP-12 RW-02 RW-03', 'C05 C35',
    'features/capture/PreviewDetails.tsx features/capture/WindowPreview.tsx',
    'window-preview', '#rw-preview-details',
    [click('#window-preview [data-rw-action="preview-details"]')],
    present={
        'normal': [], 'light': [], 'narrow': [],
        'zoom_200': [evaluate('document.documentElement.style.zoom="2";window.dispatchEvent(new Event("resize"));')],
        'keyboard_focus_escape': [{'action':'press','key':'Escape'}],
    },
    needs_mapping=['loading','error','long_text'],
    gap='详情为真实独立Sheet；复制在途/失败和长事实的同态设计映射仍待补，独立参考与正式包未验收。')
preview_details['not_applicable']={'empty','disabled'}
preview_details['targets']={'keyboard_focus_escape':'#window-preview [data-rw-action="preview-details"]'}
preview_details['assertions']={
    'normal':[{'action':'assert_text_contains','selector':'#rw-preview-details-title','value':'预览详情'}],
    'light':[{'action':'assert_design_predicate','expression':'document.documentElement.dataset.theme === "light"'}],
    'narrow':[{'action':'assert_design_predicate','expression':'window.innerWidth === 1024 && window.innerHeight === 768'}],
    'zoom_200':[{'action':'assert_design_predicate','expression':'Number.parseFloat(getComputedStyle(document.documentElement).zoom) === 2'}],
    'keyboard_focus_escape':[
        {'action':'assert_property','selector':'#rw-preview-details','name':'open','value':False},
        {'action':'assert_attribute','selector':'#window-preview [data-rw-action="preview-details"]','name':'aria-expanded','value':'false'},
        {'action':'assert_focused','selector':'#window-preview [data-rw-action="preview-details"]'},
    ],
}
SPECS['overlay:PreviewDetails:c72c668099']=preview_details

SHARED = {
'Button':('C02','ui/primitives/Controls.tsx','#select-showcase button',['hover','pressed','pending','danger','icon_only']),
'TextField':('C03','ui/primitives/Controls.tsx','#manual-source',['password','number','required_error','multiline']),
'Switch':('C04','ui/primitives/Controls.tsx','[data-settings-field="general.furigana"]',['checked','unchecked','label_click']),
'Select':('C04','ui/primitives/Select.tsx','#select-showcase',['expanded','empty_options','loading_options','error_options','disabled_options','long_options','scroll_options','unique_active','escape_restores_value']),
'Sheet':('C05','ui/primitives/Controls.tsx','#app-dialog',['focus_trap','body_scroll','footer_visible','cancel_restore_focus','nested_overlay']),
'ActionMenu':('C06','ui/primitives/Overlays.tsx','#hud-menu',['expanded','disabled_item','keyboard_arrow','escape_restore_focus']),
'AnchoredPopover':('C06','ui/primitives/Overlays.tsx','#word-popover',['anchor_collision','outside_close','escape_restore_focus']),
'PageHeader':('C09','ui/patterns/Patterns.tsx','#main-content .page-header',['actions_wrap']),
'EmptyState':('C08','ui/patterns/Patterns.tsx','#main-content .empty-content',['recovery_action']),
'Group':('C07','ui/patterns/Patterns.tsx','#settings-content .ls-section',['nested_error']),
'FormRow':('C07','ui/patterns/Patterns.tsx','#settings-content .ls-row',['label_help_error','control_overflow']),
'Notice':('C08','ui/patterns/Patterns.tsx','#main-content .status-banner',['status','alert','recovery_action']),
'StatusLabel':('C08','ui/patterns/Patterns.tsx','#main-content .pill',['warning','error_reason']),
'WindowFrame':('C34','ui/patterns/WindowFrame.tsx','#main-window',['native_drag_hit_test','window_tools_hit_test']),
'Icon':('C02','ui/icons/Icon.tsx','#select-showcase svg',['neutral_stroke','button_alignment']),
'WindowLoading':('C08','ui/patterns/WindowFrame.tsx','#main-window',['retry_keeps_error','recover_snapshot']),
}
for name,(components,paths,selector,extras) in SHARED.items():
    scenario='components' if name in ('Button','Select','Icon') else 'settings' if name in ('Switch','Group','FormRow') else 'first-run' if name=='EmptyState' else 'reading'
    steps=settings_steps('general') if name in ('Switch','Group','FormRow') else main_steps('manual') if name=='TextField' else [click('#hud-more')] if name=='ActionMenu' else [click('[data-action="open-main"]')] if name in ('PageHeader','StatusLabel','WindowFrame') else []
    # Dedicated renderer does not imply an independently approved React reference.
    present={'normal':[]} if name in ('Button','Select','Icon','TextField','Switch','ActionMenu','PageHeader','Group','FormRow','StatusLabel','WindowFrame') else {}
    SPECS['shared:'+name]=spec('共享组件 '+name,'FP-11 RW-13 RW-14 RW-15 RW-16 RW-31',components,paths,scenario,selector,steps,present,extras,gap='共享HTML材质示例只作设计来源；真实组件与每个使用容器另验。')

# Actual production subnavigation is inventoried separately from its parent.
# Guided has merged the former model tabs; their distinct production surfaces
# therefore require removal/merge obligations rather than new designs of old tabs.
for tab,title,components,extra in [
    ('purposes','用途','C27',['scope','inheritance','enabled']),
    ('connections','连接','C26 C30',['empty_connections','credential_pool','network_policy']),
    ('models','模型','C26 C28',['empty_models','model_discovery_error','manual_model_id']),
    ('parameters','模板与参数','C28',['template_variables','temperature_omit','purpose_override']),
    ('runtime','测试与运行','C29 C31',['independent_object_test','cancel_test','stale_test']),
    ('terms','术语表','C28',['empty_glossary','editing_glossary','invalid_term','save_error']),
]:
    key='subpage:ModelConfigScreen:tab:'+tab
    SPECS[key]=spec('生产配置子页 · '+title,'FP-06 MC-03 MC-05 MC-09 MC-10 MC-11 MC-12',components,'features/model-config/ModelConfigScreen.tsx features/model-config/TermSetEditor.tsx' if tab=='terms' else 'features/model-config/ModelConfigScreen.tsx','settings','#settings-content',extras=extra,needs_mapping=[*COMMON_STATES,*extra],gap='生产仍有独立'+tab+'分页；guided对应服务编辑器/用途高级设置已合并，术语表没有当前对应设计。不得把旧分页或旧v4参照自动继承为guided已对齐。')
for tab in ('grammar','dictionary'):
    steps=[click('[data-learn-action="'+('grammar-tab' if tab=='grammar' else 'dictionary-tab')+'"]')]
    SPECS['subpage:StudyScreen:tab:'+tab]=spec('学习子页 · '+tab,'FP-07 FP-08 MC-06 MC-17','C19 C20 C21 C22','features/study/StudyScreen.tsx','learning','#learning-content',steps,present={'normal':[]},extras=['fixed_old_sentence','query_or_task_error','reference_or_entry','return_from_settings'])
for dataset,section in [('body','reading'),('extraction','texts'),('hook_events','logs')]:
    selector='details[data-flat-session-section="'+section+'"]'
    SPECS['subpage:ReadingPanel:dataset:'+dataset]=spec('会话数据子页 · '+dataset,'FP-09 RW-17 RW-18 RW-19 RW-20 RW-21 RW-22','C36','features/reading/ReadingPanel.tsx features/reading/SessionRecordPanel.tsx','window-preview',selector,[click(selector+'>summary')],present={'normal':[]},extras=['independent_query','pagination','pause_follow','new_count','export','integrity_gap'],gap='生产当前页签与guided同页折叠数据集不同；数据归属/查询分页等义务保留。')
for choice in ('windows','processes'):
    steps=[click('#library-running')]
    if choice=='processes': steps.append(click('[data-gl-action="running-tab"][data-gl-value="processes"]'))
    SPECS['subpage:CaptureSheet:list:'+choice]=spec('目标来源子页 · '+choice,'GL-11 GL-12 GL-13 GL-17 GL-19','C13 C14','features/capture/CaptureSheet.tsx','existing-library','#app-dialog .gl-target-list',steps,present={'normal':[]},extras=['filter_no_match','scan_error','self_disabled','selected_identity'])
for step in ('select','streams'):
    SPECS['subpage:CaptureSheet:step:'+step]=spec('接入步骤 · '+step,'GL-14 GL-19 GL-20','C14 C15','features/capture/CaptureSheet.tsx','existing-library','#app-dialog' if step=='select' else '#connection-sheet',[click('#library-running')] if step=='select' else [],present={'normal':[]} if step=='select' else {},extras=['cancel','waiting','failure','selected'],gap='来源选择/等待候选需要当前对应设计数据与真实接入执行，不能用窗口选择成功替代收到正文。')
# An SVG icon has no asynchronous/error/keyboard state of its own. The container
# owns those behaviors; keeping them N/A avoids inventing new product features.
SPECS['shared:Icon']['not_applicable']=set(['empty','loading','error','disabled','keyboard_focus_escape'])

# Concrete existing design branches/controls. These are design replay recipes,
# not product adapters or screenshot acceptance. Each remains separately blocked
# on independent references and the two real product layers.
SPECS['main:library']['present'].update({'search_no_match':[fill('#library-search','不存在的游戏')], 'cover_missing':[{'action':'navigate','path':ENTRY,'query':{'scenario':'existing-library','theme':'dark','cover':'missing'}}], 'selected_detail':[click('[data-game="sakura"]')]})
SPECS['main:session']['present'].update({
    'empty':[{'action':'navigate_scenario','scenario':'first-run'},*main_steps('session')],
    'body_collapsed':[], 'body_expanded':[click('details[data-flat-session-section="reading"]>summary')],
    'extraction_collapsed':[], 'extraction_expanded':[click('details[data-flat-session-section="texts"]>summary')],
    'hook_collapsed':[], 'hook_expanded':[click('details[data-flat-session-section="logs"]>summary')],
    'query_no_match':[click('details[data-flat-session-section="reading"]>summary'),fill('#flat-reading-rw-session-search','不存在的原文')],
    'translation_pending':[state("app.state.current.translationStatus='pending';app.state.current.translation=''")],
    'translation_failed':[select('#review-fault','translation')],
    'translation_cancelled':[state("app.state.current.translationStatus='cancelled';app.state.current.translation=''")],
    'translation_disabled':[state("app.state.current.translationStatus='disabled';app.state.current.translation=''")],
})
SPECS['main:history']['present'].update({'game_filter':[click('#history-filter--trigger'),click('#history-filter--listbox [data-value="sakura"]')], 'locate_current':[click('[data-action="history-current"]')], 'correction_edit':[click('[data-action="edit-history"]')], 'retranslate':[click('[data-action="retranslate-history"]')], 'frozen_learning':[click('[data-action="learn-history"]')]})
SPECS['main:diagnostics']['present'].update({'no_events':[state('app.state.diagnosticEvents=[]')], 'redacted_details':[click('[data-action="export-diagnostics"]')], 'integrity_recovery_confirm':[click('[data-action="recover"]')], 'game_exited':[select('#review-fault','exit')]})
SPECS['main:diagnostics']['targets']={'redacted_details':'#app-dialog','integrity_recovery_confirm':'#app-dialog'}
SPECS['window:hud']['present'].update({**{key:[{'action':'navigate_scenario','scenario':scenario}] for key,scenario in [('plain','hud-plain'),('ruby_only','hud-ruby'),('unknown_reading','hud-unknown'),('lexical_error','hud-lexical-error'),('reduced_transparency','hud-reduced')]},
    'compact_500':[click('[data-action="toggle-hud-size"]')], 'comfortable_560':[], 'more_menu':[click('#hud-more')],
    'service_selector':[click('#hud-window [data-service-action="open"]')], 'game_exit':[select('#review-fault','exit')],
    'streaming':[state("app.state.current.translationStatus='streaming'")], 'cancelled':[state("app.state.current.translationStatus='cancelled'")],
    'hide_restore':[click('#hud-hide'),click('#dock [data-action="toggle-hud"]')],
})
SPECS['window:hud']['targets']={'more_menu':'#hud-menu','service_selector':'#translation-service-picker'}
SPECS['window:learning']['present'].update({'dictionary_tab':[click('[data-learn-action="dictionary-tab"]')], 'unconfigured_grammar':[], 'old_sentence_fixed':[evaluate('window.LunaDesign.advance()')]})
SPECS['window:settings']['present'].update({'search_match':[fill('[data-settings-field="search"]','显示')], 'return_main_focus':[click('[data-settings-action="back-main"]')]})
SPECS['window:settings']['targets']={'return_main_focus':'#main-window'}
SPECS['settings:data']['present'].update({'safe_export':[click('[data-settings-action="export"]')], 'import_preflight':[click('[data-settings-action="import"]')], 'import_conflict':[click('[data-settings-action="import"]')], 'cancel_import':[click('[data-settings-action="import"]'),click('[data-settings-action="cancel-import"]')], 'apply_import_draft':[click('[data-settings-action="import"]'),click('[data-settings-action="apply-import"]')], 'restore_saved':[click('[data-settings-action="restore-saved"]')]})
SPECS['settings:data']['targets']={'safe_export':'#app-dialog','restore_saved':'#app-dialog'}
SPECS['settings:diagnostics']['present'].update({'independent_purpose_status':[], 'redacted_diagnostic':[click('[data-settings-action="diagnostic"]')]})
SPECS['settings:display']['present'].update({'comfortable':[], 'compact':[state("app.state.settings.preferences.preset='compact'")], 'font_scale':[state('app.state.settings.preferences.fontScale=2')], 'reduced_material':[state("app.state.settings.preferences.appearance='reduced'")], 'shortcut_draft':[fill('[data-settings-field="display.hideShortcut"]','⌥ ⌘ J')], 'restore_position':[click('#settings-content [data-rw-action="reset-hud"]')]})
SPECS['shared:Button']['present']['disabled']=[]
SPECS['shared:Button']['targets']={'disabled':'#select-showcase .rw-component-grid > article:nth-child(2) button:disabled'}
SPECS['shared:Select']['present'].update({'disabled':[], 'empty_options':[click('#showcase-no-options--trigger')], 'loading_options':[], 'error_options':[], 'disabled_options':[click('#showcase-normal--trigger')], 'long_options':[click('#showcase-normal--trigger')]})
SPECS['shared:Select']['targets']={'normal':'#showcase-normal--trigger','disabled':'#showcase-disabled--trigger','empty_options':'#showcase-no-options--listbox .luna-select-empty','loading_options':'#showcase-loading--trigger','error_options':'#showcase-error--trigger','disabled_options':'#showcase-normal--listbox [aria-disabled="true"]','long_options':'#showcase-normal--option-2'}
SPECS['shared:Select']['assertions']={
    'disabled':[{'action':'assert_disabled','selector':'#showcase-disabled--trigger'}],
    'loading_options':[{'action':'assert_attribute','selector':'#showcase-loading--trigger','name':'data-select-state','value':'loading'}],
    'error_options':[{'action':'assert_attribute','selector':'#showcase-error--trigger','name':'aria-invalid','value':'true'}],
    'disabled_options':[{'action':'assert_attribute','selector':'#showcase-normal--listbox [aria-disabled="true"]','name':'aria-disabled','value':'true'}],
    'long_options':[{'action':'assert_text_contains','selector':'#showcase-normal--option-2','value':'GameN7_Steam.exe'}],
}
SPECS['window:settings']['assertions']={'return_main_focus':[{'action':'assert_focused','selector':'#main-open-settings'}]}
for page,s in SPECS.items():
    if 'normal' in s['present'] and 'normal' not in s['needs_mapping']:
        s['present']['light']=[]

# Lifecycle and responsibility are independent from the existence of a replay.
# No replay means needs_mapping, never proof that the authored design is absent.
for s in SPECS.values():
    s['retired']=None
    s['not_applicable']=set(s.get('not_applicable',()))
    s['responsibility_reason']='状态归属由当前组件props、页面职责及范围确定；未映射不推断为缺设计。'
SPECS['overlay:PreviewDetails:c72c668099']['responsibility_reason']='详情仅在已有候选或会话入口下打开；缺事实逐字段明示，不存在独立空内容或禁用整窗。复制有独立在途/失败状态，因此loading/error仍是必须映射和验证的义务；长路径和缩放由本Sheet负责。'
PRESENTATIONAL={'Icon','PageHeader','Group','FormRow','EmptyState','StatusLabel','WindowFrame'}
NO_ASYNC={'Button','TextField','Switch','Sheet','ActionMenu','AnchoredPopover',*PRESENTATIONAL}
for name in SHARED:
    s=SPECS['shared:'+name]
    if name in PRESENTATIONAL:
        s['not_applicable'].update(('empty','loading','error','disabled','keyboard_focus_escape'))
    elif name in NO_ASYNC:
        s['not_applicable'].update(('loading','error'))
    if name=='Button': s['not_applicable'].discard('loading')  # pending prop
    if name in ('Switch','Sheet','AnchoredPopover','ActionMenu'): s['not_applicable'].add('empty')
    if name in ('Sheet','AnchoredPopover'): s['not_applicable'].add('disabled')
    if name=='WindowLoading': s['not_applicable'].update(('empty','disabled'))
    if name=='Icon': s['not_applicable'].add('long_text')
    s['responsibility_reason']='依据 '+s['paths'][0]+' 的实际props；内容/任务错误由调用容器拥有，不给纯展示组件制造异步状态。'
for page in ('window:main','window:settings'):
    SPECS[page]['not_applicable'].update(('empty','disabled'))
# The approved guided target merges the former six entity tabs and three record
# tabs. Keep negative removal obligations, never request replacement old pages.
for key,s in SPECS.items():
    if key.startswith('subpage:ModelConfigScreen:tab:'):
        # Source capture responsibility is independent of the live AST fallback.
        s['source_container']='container'
        tab=key.rsplit(':',1)[1]
        section={'purposes':'[data-guided-purpose-policy]','connections':'[data-guided-section="editor"]','models':'[data-guided-section="editor"]','parameters':'[data-guided-purpose-policy]','runtime':'[data-guided-section="editor"]','terms':'[data-guided-purpose-policy]'}[tab]
        s['retired']={'status':'retired_production_surface','parent_surface':'settings:translation','replacement_section':section,
            'replacement_description':'服务必要配置编辑器及用途高级设置；terms编辑位置仍需明确映射，不复原旧术语表页签。' if tab=='terms' else '在guided服务配置/用途高级设置内完成对应职责，不保留旧实体分页。',
            'forbidden_production_selectors':['[aria-label="模型配置实体"]'],'policy':'merge_into_current_guided_sections_never_recreate_legacy_tab'}
    if key.startswith('subpage:ReadingPanel:dataset:'):
        s['source_container']='main'
        dataset=key.rsplit(':',1)[1];section={'body':'reading','extraction':'texts','hook_events':'logs'}[dataset]
        s['retired']={'status':'retired_production_surface','parent_surface':'main:session','replacement_section':'details[data-flat-session-section="'+section+'"]',
            'replacement_description':'当前阅读下方同页独立折叠数据集，默认折叠，查询/分页/跟随状态独立。',
            'forbidden_production_selectors':['[role="tablist"][aria-label="当前会话内容"]'],'policy':'merge_into_current_guided_sections_never_recreate_legacy_tab'}
    if s['retired']:
        s['present']={'retired_surface_absent':[]};s['extras']=[];s['selector']=s['retired']['replacement_section'];s.setdefault('assertions',{})['retired_surface_absent']=[{'action':'assert_absent','selector':v} for v in s['retired']['forbidden_production_selectors']];s['gap']='旧生产页签需要删除/合并；只验证旧导航消失和新section映射，不要求重新设计旧页签。'
        s['not_applicable']=set()
# Explicit source-audited gaps. Every missing claim has a separate fixed source
# anchor and reason. Other unmapped states are intentionally NOT in this table.
VERIFIED_DESIGN_GAPS={}
# Observable branch-specific targets and predicates, distinct from later pixel,
# clipping and native measurements. Recipes without these remain needs_mapping.
SPECS['main:library'].setdefault('targets',{}).update({'normal':'#main-content .game-grid','empty':'#main-content .empty-content','search_no_match':'#library-search','cover_missing':'#main-content .game-grid','selected_detail':'#main-content .selected-game-details'})
SPECS['main:session'].setdefault('targets',{}).update({'normal':'#main-content .reading-unit','empty':'#main-content .empty-content','translation_pending':'#main-content .reading-unit .translation','translation_failed':'#main-content .reading-unit .translation','translation_cancelled':'#main-content .reading-unit .translation','translation_disabled':'#main-content .reading-unit .translation'})
SPECS['main:history'].setdefault('targets',{}).update({'normal':'#main-content .history-layout','empty':'#main-content .empty-content','error':'#main-content .history-edit-error','correction_edit':'#history-edit','frozen_learning':'#learning-window'})
SPECS['main:manual'].setdefault('targets',{}).update({'normal':'#manual-source','empty':'#manual-source','loading':'#main-content .manual-result','error':'#main-content .manual-meta'})
for page,s in SPECS.items():
    assertions=s.setdefault('assertions',{})
    for name in s['present']:
        if name not in assertions:
            predicate=None
            if name=='light': predicate="document.documentElement.dataset.theme === 'light'"
            elif page.startswith('settings:'):
                category=SETTINGS[page.split(':')[1]][3]
                predicate='window.LunaDesign.state.settings.category === '+json.dumps(category)
            elif page.startswith('main:'): predicate='window.LunaDesign.state.page === '+json.dumps(page.split(':')[1])
            elif page.startswith('window:'): predicate='window.LunaDesign.state.windows['+json.dumps(page.split(':')[1])+'] === true'
            if predicate: assertions[name]=[{'action':'assert_design_predicate','expression':predicate}]
# Specific outcomes must not be represented by mere visibility of the page shell.
for name,field,value in [('loading','manualStatus','pending'),('error','manualStatus','failed')]:
    SPECS['main:manual']['assertions'][name]=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.'+field+' === '+json.dumps(value)}]
for name,status in [('translation_pending','pending'),('translation_failed','failed'),('translation_cancelled','cancelled'),('translation_disabled','disabled')]:
    SPECS['main:session']['assertions'][name]=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.current.translationStatus === '+json.dumps(status)}]
SPECS['main:history']['assertions']['game_filter']=[{'action':'assert_design_predicate','expression':"window.LunaDesign.state.historyFilter === 'sakura'"}]
SPECS['main:history']['assertions']['error']=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.historyEditError === true'}]
for page,s in SPECS.items():
    for name,steps in s['present'].items():
        assertions=s['assertions'].setdefault(name,[])
        for action in [*s['steps'],*steps]:
            if action['action']=='fill': assertions.append({'action':'assert_value','selector':action['selector'],'value':action['value']})

# Concrete state assertions for already replayed design controls.
for section,prefix in [('reading','body'),('texts','extraction'),('logs','hook')]:
    for suffix,opened in [('collapsed',False),('expanded',True)]:
        name=prefix+'_'+suffix
        SPECS['main:session']['targets'][name]='details[data-flat-session-section="'+section+'"]'
        SPECS['main:session']['assertions'][name]=[{'action':'assert_property','selector':SPECS['main:session']['targets'][name],'name':'open','value':opened}]
PREDICATES={
 'main:library':{'search_no_match':"document.querySelectorAll('#main-content .game-card').length === 0"},
 'main:history':{'locate_current':'window.LunaDesign.state.historySelection === window.LunaDesign.state.current.recordId'},
 'main:diagnostics':{'no_events':'window.LunaDesign.state.diagnosticEvents.length === 0','game_exited':"window.LunaDesign.state.session.status === 'exited'"},
 'window:hud':{'plain':'window.LunaDesign.state.settings.general.segmentation === false && window.LunaDesign.state.settings.general.furigana === false','ruby_only':'window.LunaDesign.state.settings.general.segmentation === false && window.LunaDesign.state.settings.general.furigana === true','unknown_reading':"window.LunaReviewGaps.lexicalMode() === 'unknown'",'lexical_error':"window.LunaReviewGaps.lexicalMode() === 'error'",'compact_500':"window.LunaDesign.state.settings.preferences.preset === 'compact'",'comfortable_560':"window.LunaDesign.state.settings.preferences.preset === 'comfortable'",'streaming':"window.LunaDesign.state.current.translationStatus === 'streaming'",'cancelled':"window.LunaDesign.state.current.translationStatus === 'cancelled'",'reduced_transparency':"window.LunaDesign.state.settings.preferences.appearance === 'reduced'"},
 'window:learning':{'dictionary_tab':"window.LunaDesign.state.learning.tab === 'dictionary'",'old_sentence_fixed':'window.LunaDesign.state.learning.unit.id !== window.LunaDesign.state.current.id','unconfigured_grammar':"window.LunaSettings.effective(window.LunaDesign.state.settings,'grammar',window.LunaDesign.state.learning.unit.gameId).enabled === false"},
 'settings:data':{'import_preflight':"window.LunaDesign.state.settings.importState === 'preflight'",'import_conflict':"window.LunaDesign.state.settings.importState === 'preflight'",'cancel_import':"window.LunaDesign.state.settings.importState === 'idle'",'apply_import_draft':'window.LunaDesign.state.settings.dirty === true'},
 'settings:display':{'comfortable':"window.LunaDesign.state.settings.preferences.preset === 'comfortable'",'compact':"window.LunaDesign.state.settings.preferences.preset === 'compact'",'font_scale':'window.LunaDesign.state.settings.preferences.fontScale === 2','reduced_material':"window.LunaDesign.state.settings.preferences.appearance === 'reduced'"},
}
for page,states in PREDICATES.items():
    for name,predicate in states.items(): SPECS[page]['assertions'].setdefault(name,[]).append({'action':'assert_design_predicate','expression':predicate})
for page in ('main:manual',):
    for name in ('normal','empty'): SPECS[page]['assertions'].setdefault(name,[]).append({'action':'assert_value','selector':'#manual-source','value':''})

# Retiring the old terms tab must not erase its MC-09 business responsibility.
SPECS['settings:translation']['extras'].append('term_editing_mapping')
SPECS['settings:translation'].setdefault('state_mapping_overrides',{})['term_editing_mapping']={
    'source_case_ids':['FP-06','MC-09'], 'component_ids':['C28'],
    'component_paths':[PREFIX+'features/model-config/TermSetEditor.tsx',PREFIX+'features/model-config/ModelConfigScreen.tsx']}
SPECS['window:learning']['present']['disabled']=[state("app.state.settings.blockedConnections.cloud=true")]
SPECS['window:learning']['targets']={'disabled':'#learning-content .lp-setup','empty':'#learning-content .lp-empty'}
SPECS['window:learning']['assertions']['disabled']=[{'action':'assert_text_contains','selector':'#learning-content .lp-setup','value':'服务连接已停用'}]
SPECS['window:learning']['assertions']['empty']=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.learning.unit === null'}]
SPECS['window:hud']['assertions']['error']=[{'action':'assert_design_predicate','expression':"window.LunaDesign.state.current.translationStatus === 'failed'"},{'action':'assert_text_contains','selector':'#hud-translations','value':'翻译请求失败'}]
SPECS['window:hud']['assertions']['disabled']=[{'action':'assert_design_predicate','expression':"window.LunaDesign.state.current.translationStatus === 'disabled'"},{'action':'assert_text_contains','selector':'#hud-translations','value':'翻译尚未启用'}]
SPECS['window:hud']['assertions']['game_exit']=[{'action':'assert_design_predicate','expression':"window.LunaDesign.state.session.status === 'exited'"},{'action':'assert_text_contains','selector':'#hud-state','value':'游戏已退出'}]
SPECS['window:hud']['present']['hide_restore']=[click('#hud-hide'),{'action':'assert_property','selector':'#hud-window','name':'hidden','value':True},click('#dock [data-action="toggle-hud"]')]
SPECS['window:hud']['assertions']['hide_restore']=[{'action':'assert_property','selector':'#hud-window','name':'hidden','value':False}]
SPECS['main:session']['targets']['error']='#main-content .status-banner.warning'
SPECS['main:session']['assertions']['error']=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.session.integrityLost === true'},{'action':'assert_text_contains','selector':'#main-content .status-banner.warning','value':'接收发生中断'}]
SPECS['main:session']['targets']['query_no_match']='details[data-flat-session-section="reading"] .flat-empty'
SPECS['main:history']['assertions']['retranslate']=[{'action':'assert_design_predicate','expression':"window.LunaDesign.state.historySelection.includes(':retry:')"}]
SPECS['main:history']['assertions']['frozen_learning']=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.learning.unit.recordId === window.LunaDesign.state.historySelection'}]
SPECS['settings:display']['present']['restore_position']=[evaluate("const app=window.LunaDesign,hud=document.getElementById('hud-window');app.state.draggedWindows.hud={left:12,top:12};hud.style.left='12px';hud.style.top='12px';"),{'action':'assert_design_predicate','expression':"document.getElementById('hud-window').style.top === '12px'"},click('#settings-content [data-rw-action="reset-hud"]')]
SPECS['settings:display']['assertions']['restore_position']=[{'action':'assert_design_predicate','expression':"!window.LunaDesign.state.draggedWindows.hud && document.getElementById('hud-window').style.top === '450px'"}]

# A generated receipt sentence is not evidence of redaction. Until there is a
# typed payload/visible-output audit, keep these privacy states needs_mapping.
for page,name in [('settings:diagnostics','redacted_diagnostic'),('main:diagnostics','redacted_details'),('settings:data','safe_export')]:
    SPECS[page]['present'].pop(name,None)
# This state is only the preflight conflict handling choice, not successful
# replacement/import. It must visibly differ from the initial preflight state.
STATE_LABELS['import_conflict']='导入预检冲突：选择替换（未应用）'
SPECS['settings:data']['present']['import_conflict']=[click('[data-settings-action="import"]'),click('#design-select-importConflict--trigger'),click('#design-select-importConflict--listbox [data-value="replace"]')]
SPECS['settings:data']['targets']['import_conflict']='#design-select-importConflict--trigger'
SPECS['settings:data']['assertions']['import_conflict']=[{'action':'assert_design_predicate','expression':"window.LunaDesign.state.settings.importState === 'preflight' && window.LunaDesign.state.settings.importConflict === 'replace'"},{'action':'assert_text_contains','selector':'#design-select-importConflict--trigger','value':'替换同名草稿'}]
for page,s in SPECS.items():
    if page.startswith('subpage:StudyScreen:tab:'):
        value=page.rsplit(':',1)[1]
        s['assertions']['normal']=[{'action':'assert_design_predicate','expression':'window.LunaDesign.state.learning.tab === '+json.dumps(value)}]

def _specific_recipe_outcome(s,name):
    if name in ('normal','light','retired_surface_absent'): return True
    for assertion in s.get('assertions',{}).get(name,[]):
        if assertion['action']!='assert_design_predicate': return True
        expression=assertion.get('expression','')
        if not any(expression.startswith(prefix) for prefix in ('window.LunaDesign.state.page ===','window.LunaDesign.state.windows[','window.LunaDesign.state.settings.category ===','document.documentElement.dataset.theme ===')): return True
    selector=s.get('targets',{}).get(name,'')
    return bool(selector and selector not in ('#app-dialog','#main-window','#settings-window','#hud-window','#learning-window') and any(part in selector for part in ('.empty-content','.lp-empty','#history-edit','.history-edit-error','.flat-empty')))
# Registration cannot promote a special state on mere window/category visibility.
# Retain the obligation, but drop the incomplete recipe into needs_mapping.
for s in SPECS.values():
    for name in list(s['present']):
        if not _specific_recipe_outcome(s,name): s['present'].pop(name)

# The current composition overrides only history; the default keeps guided.
# Each operation names the actual hx surface. Missing detail/revision-conflict
# states remain unmapped; list loading and saveError cannot stand in for them.
history=SPECS['main:history']
history.update(entry=HISTORY_ENTRY,query={'historyState':'default'},steps=[],selector='#hx-root',
    needs_mapping=['detail_loading','detail_error','correction_conflict'],
    gap='历史目标已纳入完整批准组合；正式HistoryScreen仍须迁移会话目录/记录独立查询与R79轻量操作，详情加载/错误及revision冲突仍待独立映射。')
history['present']={
    'normal':[], 'empty':[], 'loading':[],
    'error':[evaluate('window.LunaHistoryDesign.setupScenario({saveError:true});'),click('#hx-edit'),click('#hx-save')],
    'disabled':[click('#hx-edit')], 'long_text':[], 'light':[], 'narrow':[],
    'zoom_200':[evaluate('document.documentElement.style.zoom="2";window.dispatchEvent(new Event("resize"));')],
    'keyboard_focus_escape':[evaluate('const api=window.LunaHistoryDesign;await api.chooseGame("sakura");api.selectRecord(api.getState().records.find(r=>r.designFixture && r.tokens?.[0]?.meaning).recordId);'),click('#hx-detail [data-hx-token-index="0"]'),visible('#hx-word-popover'),{'action':'press','key':'Escape'}],
    'game_filter':[evaluate('await window.LunaHistoryDesign.chooseGame("sakura");')],
    'session_filter':[evaluate('const api=window.LunaHistoryDesign;await api.chooseSession(api.getState().sessions.find(s=>s.recordCount>0).id);')],
    'pagination':[evaluate('await window.LunaHistoryDesign.loadMoreRecords();await window.LunaHistoryDesign.loadMoreSessions();')],
    'locate_current':[click('#hx-locate-current')], 'correction_edit':[click('#hx-edit')],
    'retranslate':[evaluate('const services=window.LunaTranslationServices,item=services.list().find(row=>row.available);if(!item)throw new Error("No available page-local design service");services.open({origin:"main",scope:"global"});if(!services.choose(item.id))throw new Error("Explicit design-service selection failed");'),click('#hx-retranslate')], 'frozen_learning':[click('#hx-learn')],
}
history['state_queries']={'empty':{'historyState':'empty'},'loading':{'historyState':'initial'},'long_text':{'historyState':'long'}}
history['targets']={**{n:'#hx-root' for n in history['present']},'empty':'#hx-record-list .hx-empty','loading':'#hx-record-list .hx-skeleton','error':'#hx-detail .hx-error','disabled':'#hx-edit-input','long_text':'#hx-detail .hx-source','correction_edit':'#hx-edit-input','frozen_learning':'#learning-window'}
history['assertions']={n:[{'action':'assert_design_predicate','expression':expression}] for n,expression in {
    'normal':'window.LunaHistoryDesign.getState().recordIds.length > 0',
    'empty':'window.LunaHistoryDesign.getState().recordIds.length === 0 && window.LunaHistoryDesign.getState().recordsStatus === "end"',
    'loading':'window.LunaHistoryDesign.getState().recordsStatus === "loading" && window.LunaHistoryDesign.getState().selectedRecordId === null',
    'error':'window.LunaHistoryDesign.getState().editError === true && window.LunaHistoryDesign.getState().editing !== null',
    'disabled':'window.LunaHistoryDesign.getState().editing !== null && document.querySelector("#hx-refresh").disabled && document.querySelector("#hx-locate-current").disabled && document.querySelector("#hx-records-more").disabled && document.querySelector("#hx-sessions-more").disabled && document.querySelector("#hx-game-filter").disabled && document.querySelector("#hx-session-filter").disabled',
    'long_text':'window.LunaHistoryDesign.getState().reviewState === "long" && window.LunaHistoryDesign.getState().selectedRecord.source.length > 300',
    'light':'document.documentElement.dataset.theme === "light"',
    'narrow':'window.innerWidth === 740 && window.innerHeight === 1200',
    'zoom_200':'Number.parseFloat(getComputedStyle(document.documentElement).zoom) === 2',
    'keyboard_focus_escape':'window.LunaHistoryDesign.getState().wordCard === null && document.querySelector("#hx-word-popover").hidden === true',
    'game_filter':'window.LunaHistoryDesign.getState().gameId === "sakura" && window.LunaHistoryDesign.getState().visibleScope.gameId === "sakura"',
    'session_filter':'window.LunaHistoryDesign.getState().sessionId !== "all" && window.LunaHistoryDesign.getState().records.every(r=>r.sessionId === window.LunaHistoryDesign.getState().sessionId)',
    'pagination':'window.LunaHistoryDesign.getState().recordCursor > 50 && window.LunaHistoryDesign.getState().sessionCursor > 50 && new Set(window.LunaHistoryDesign.getState().recordIds).size === window.LunaHistoryDesign.getState().recordIds.length && new Set(window.LunaHistoryDesign.getState().sessionIds).size === window.LunaHistoryDesign.getState().sessionIds.length',
    'locate_current':'window.LunaHistoryDesign.getState().gameId === "all" && window.LunaHistoryDesign.getState().sessionId === "all" && window.LunaHistoryDesign.getState().selectedRecordId === window.LunaDesign.state.current.recordId',
    'correction_edit':'window.LunaHistoryDesign.getState().editing === window.LunaHistoryDesign.getState().selectedRecordId',
    'retranslate':'window.LunaHistoryDesign.getState().selectedRecordId.includes(":hx-retry:")',
    'frozen_learning':'window.LunaDesign.state.learning.unit.recordId === window.LunaHistoryDesign.getState().selectedRecordId',
}.items()}

# Exact frozen complete61 sources plus the actual CSS wallpaper dependency.
# Approval records are separate authority inputs; no approved byte is refreshed.
DESIGN_FIXTURE_SHA256 = {'docs/architecture/model-service-registry-design.json': '32d5c81aa3480a962c94fa59922a3faed9cee897ac2479430998e9d74a07a9f2', 'docs/architecture/service-configuration-presentation.json': '9ee94eae5975a0891c51db50b401130779d03b410203c7f95d20092d052ae3a6', 'docs/design/first-product-v1/app.js': 'bc1112ca97c308f4dd4c89833c1b78a689079ab77534d2b34ad894389b435e6e', 'docs/design/first-product-v1/assets/manifest.json': '46525874239a21d1e597d059d349031d13a26a6cb2821f487cf2a5e7978702b9', 'docs/design/first-product-v1/assets/mo-01.png': '62f47ecac204421eac9acb47db0069cd039cef9d8629b3c6d9eaafcd33b6564a', 'docs/design/first-product-v1/assets/mo-02.png': '743f0b0edc34a1ffafa90074d2e697dc6a008433b4b2ed1fe8492f478424779e', 'docs/design/first-product-v1/assets/mo-03.png': 'd8bacae8805d689121c3d7ec2996902985a35010734ff08411ae699e6bf00053', 'docs/design/first-product-v1/assets/mo-04.png': '14caeeb59d3143e8ab750963ea7915f3e85c23de6a92b550ee0338c5ec2d5810', 'docs/design/first-product-v1/assets/never7-01.png': 'd16affbf86ad32b809b896322bca03e6f912fcde4ec5a8127a26b75937a701f8', 'docs/design/first-product-v1/assets/never7-02.png': '7348514360a4ab10a3b832ac5fe18d80b667939670a30440bbf8f96e6109fcf1', 'docs/design/first-product-v1/assets/never7-03.png': '6d84534b0e900c5af1f28473c8c5a5b996bda450253635d4b70636a435c0a64d', 'docs/design/first-product-v1/assets/never7-04.png': 'b365073b0b3ecc12004103366fd7142167f60f161446b14a21f34f62a97ef236', 'docs/design/first-product-v1/assets/never7-05.png': 'd1d54740b88b638e2785a499d6dee9205db35ccbccf14bb59f9a978fc9028d64', 'docs/design/first-product-v1/assets/sakura-01.png': 'd5521c9537f2d4774d514c74a75054fc37f70d4307f6f64673eeeec3dd1a1d01', 'docs/design/first-product-v1/assets/sakura-02.png': '7736694d1a639717b022c382474e3caa5c94e6a651a7bc3192c14c6dab28eb0e', 'docs/design/first-product-v1/assets/sakura-03.png': '67e5089f892dfadc78470503af53083c8bb2df4ba140751ef99d6e9b3fed0274', 'docs/design/first-product-v1/assets/sakura-04.png': 'dd960778bf52dfb09fa2284f1d94b718c935ff619fe077ef5281ea311d7cfd66', 'docs/design/first-product-v1/assets/sakura-05.png': '800712d20509877d692dc56c4d9004e45e30b05c414eb3efed6ab495db7cc1fc', 'docs/design/first-product-v1/assets/sr-01.png': '74fcddfca3ededa43788aae6a29b2d4201ee7c57817735bcef3bb2badfdce697', 'docs/design/first-product-v1/assets/sr-02.png': 'a47c98c24ebd19979e1c911fba8cf70964f2981209cd22845b0b3c913e1ff688', 'docs/design/first-product-v1/assets/sr-03.png': '95aa53ba5d30e736805189024a1518b5ad7746a5e33867064465e0786773ff9a', 'docs/design/first-product-v1/assets/sr-04.png': '8e07cccc241100ac3c17b705a8c16e919d3d07dddd5a2f8d84dc7c40e9161151', 'docs/design/first-product-v1/assets/sr-05.png': 'fed3dc9f1e77ff9bebe71e564a7c1aaa2106fb3f5de5cb768f58f11099d28246', 'docs/design/first-product-v1/assets/wallpaper-frame.png': '64f704c02b5172f7b18ae09e683c883f6f0913b4ea89b7399f975e9938cd6a8f', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/flat-layout.css': 'e69a542178b507ad8470468fd0422bff6c421b4cb983dac5afc14ddc8a0a5a6c', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/flat-layout.js': '63588bb1b90c47a23c057df0471b8cdd66a306055b701c1f1709b831a4983736', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/index.html': '2e1e5e4db68cd1df5952c4fe02286a66f3e2f63a01dd31f37abd3e16c0ffd86a', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/local-translation-design.css': 'bf24a826f5039d939e43796861637cffa90dff7c5eba5faff36fc0f730002143', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/local-translation-design.js': '0a24585c3135706d65adb73f7d693cee20712d9800cff2bc3959e7fea19a33ed', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/manifest.json': 'e89e788594dec5440b5e25273b5417bf2a64517d8df9fba253d35709372e897d', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/service-picker.css': 'd95b228ca91b1ad39e9862f8338012f3e5080f6e381dd5034d11ffc970c8094e', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/service-picker.js': '0c19283b5e75b3dfcb28aee76cd835a4aaf3c22fd23d8e2fe18ca21ffadb43dc', 'docs/design/first-product-v1/candidates/flat-service-session-20261030/settings.js': 'ea8b67fc78d90179e4928cd2877cc5701ba8a9fe76672431d9f742a73c2107c0', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/flat-layout.css': 'adb603b38009ca5d329696a6cca3170eeaf1f86abeef5fcbc1099148ef1a7a66', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/flat-layout.js': '13ad561888ed4430629d228c0c9bbd5e2ac597f38ad3623c00b68f27fd35b5ee', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/guided-catalog.js': '649c986b6e8fa152abecc84adf9fdf8e58aa3cf503309da0b0ae9bd22b14da33', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/guided-config.js': '51aee0f8b2b8df252972c91cb47c0cc5c2b016dfdd51f5032d83f7187da87c76', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/index.html': '1a8269771e23e2ecfb577e7666409947dd32ce50e135940d90b7d6e11d23ecaa', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/local-translation-design.css': 'bf24a826f5039d939e43796861637cffa90dff7c5eba5faff36fc0f730002143', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/local-translation-design.js': '0a24585c3135706d65adb73f7d693cee20712d9800cff2bc3959e7fea19a33ed', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/manifest.json': '3e1d23cc8486706ae4027d1766e0599a41c3b1eca4c06869a9a132803beb0233', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/service-picker.css': 'd95b228ca91b1ad39e9862f8338012f3e5080f6e381dd5034d11ffc970c8094e', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/service-picker.js': '9ce3ca72a9690d2c5eb35fff9cd0f3e524bc10e71c92d1e19c44c507803ede48', 'docs/design/first-product-v1/candidates/guided-service-reading-20261030/settings.js': '92667203cc0d868361f0d2981757cbef240c0643db750deae7c79d90c964d981', 'docs/design/first-product-v1/candidates/history-session-paging-20261031/history.css': 'd9fc1490003359e745649904edb88b14e4f59b505ec63a27f85201523e9dfa6e', 'docs/design/first-product-v1/candidates/history-session-paging-20261031/history.js': '928bf2981914a4953fc05fe5f5557c4ccc8246586120039a0c26084a2ca81dce', 'docs/design/first-product-v1/candidates/history-session-paging-20261031/index.html': '6c2cdfa3478822d8be6fed973b126cee82805b8a4a484af29e0df80155eab236', 'docs/design/first-product-v1/candidates/history-session-paging-20261031/manifest.json': '135289b6204f5bcf178f340e9e3fdff3f1384a5b51e154cd4a9bd6e7cdccfc34', 'docs/design/first-product-v1/controls.css': '5fe9a124ef79694d7bdf9a4c337aa65097f422b795abc4881e11160690796480', 'docs/design/first-product-v1/controls.js': '9ce3926e2e81e5200a1842fb4cbe78dd770683ccec8bd5f773e21c03c06dee17', 'docs/design/first-product-v1/data.js': '5ebce1b36e9f6b17a14b6f232f8bd1760ce0a8765a3138592bb8f194b62b26f9', 'docs/design/first-product-v1/game-library.css': '02be90b679a29aecc7416081fac3fdabc7b034392cff7267de00b6d82e1c1e0d', 'docs/design/first-product-v1/game-library.js': 'ec58d3d4e365b07bd9324525163374275c9966fbd8cf558fa14a5459b5ac8c42', 'docs/design/first-product-v1/icons.js': '4dab92462e14e85bbd541ebdaf81c669d8e078fe8b5461b3cf5125ad219423da', 'docs/design/first-product-v1/index.html': '49a2bbac53caec886bbd1487dab697f6037680761eca98a00c7b21248086d957', 'docs/design/first-product-v1/learning.css': 'ade9b978f3564086921b9e91ab332fbb57cd1130ef71923455e340642d6b0b74', 'docs/design/first-product-v1/learning.js': 'f4ac11986b695ee4be6e868a7cc006b7bc67e7ac5a907c0a79baf2f7dccc8304', 'docs/design/first-product-v1/material-tokens.css': '3406083383b5a58d2045fdcbbb26624b04b7f49fe3184599719826a207457a03', 'docs/design/first-product-v1/review-gaps.css': '5c1e3082ab2b993c3a88914dbfa6c1db61f7456aa6ecfba06f98e2b2f0dfbadd', 'docs/design/first-product-v1/review-gaps.js': '4f1f44f3a611e0aca7d783690bb3956301085db7412ade294d036662cbb8909e', 'docs/design/first-product-v1/settings.css': 'e1dbd6df0b7a565afbbd3c0ed4d03d6b2d8f7bbb62c989e7aa8f43719fb6f4e8', 'docs/design/first-product-v1/styles.css': 'f8d6b3ac6a744e3936f5938ab8f68b1534de7183737b901eb98d0b34b33bd7d6'}


def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def identity(value): return hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def _unique(pairs):
    result={}
    for key,value in pairs:
        if key in result: raise ValueError('duplicate JSON key: '+key)
        result[key]=value
    return result

def _read(path):
    return json.loads(Path(path).read_text(),object_pairs_hook=_unique,parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-JSON constant: '+value)))

def _safe(root, value):
    if not isinstance(value,str) or not value or value.startswith('/') or any(x in value.split('/') for x in ('','..','.')) or any(ord(c)<32 or ord(c)==127 for c in value) or any(c in value for c in ('\\',':','?','#')): raise ValueError('safe repository path required')
    p=Path(root).absolute()
    if p.is_symlink(): raise ValueError('symlink root forbidden')
    for part in value.split('/'):
        p=p/part
        if p.is_symlink(): raise ValueError('symlink source/ancestor forbidden')
    p.resolve().relative_to(Path(root).resolve())
    return p

def inventory(root, *, production_dependencies=False):
    command=['node',str(Path(__file__).with_name('page-inventory.mjs')),'--root',str(Path(root).resolve())]
    if production_dependencies: command.append('--production-dependencies')
    result=subprocess.run(command,cwd=root,text=True,capture_output=True,timeout=45)
    if result.returncode: raise ValueError('TypeScript AST inventory failed: '+result.stderr[:1000])
    data=json.loads(result.stdout)
    if data.get('errors'): raise ValueError('; '.join(data['errors']))
    return data

def _state_names(s):
    names=tuple(dict.fromkeys((*COMMON_STATES,*s['extras'])))
    if s.get('preserve_retired_states'): return (*names,'retired_surface_absent')
    return ('retired_surface_absent',) if s.get('retired') else names
def _state_id(page, name): return 'all-page-'+re.sub('[^a-zA-Z0-9_-]+','-',page).lower()+'-'+name.replace('_','-')
def _applicable(s, name): return name not in s.get('not_applicable',set()) and not (s['static'] and name in ('empty','loading','error','disabled'))
def _state_mapping(page, s, name):
    return {'id':_state_id(page,name),'scenario':s['scenario'],'source_case_ids':s['cases'],
            'component_ids':s['components'],'component_paths':s['paths'],'owners':['UI','QA','PL'], 'required_layers':LAYERS.copy(),**s.get('state_mapping_overrides',{}).get(name,{})}

SPECS['overlay:ModelConfigScreen:5fb902a57e'] = spec('添加现有协议服务','FP-06','C04 C05 C26 C31','features/model-config/ModelConfigScreen.tsx','settings','#app-dialog',settings_steps('translation'),needs_mapping=COMMON_STATES,gap='当前新增三协议服务弹层需逐态独立设计映射和参照；不能冒充完整品牌目录向导，十条状态记录均保留；七项适用义务不得凭登记通过产品。')
SPECS['overlay:ModelConfigScreen:5fb902a57e'].update(retired=None,not_applicable={'empty','loading','error'},responsibility_reason='ModelConfigScreen.addService仅同步创建本地草稿并关闭Sheet，固定三协议且不调用port/await；空白名称归确认disabled，加载与错误由宿主保存测试负责。Sheet仍负责禁用确认、长文、主题、窄窗、200%与键盘取消。')

# A route disappearing does not remove its functional/state obligations. These
# three migrations preserve every original state descriptor and add a distinct
# absence assertion. The lineage freezes the pre-migration809 closure; unlike
# the older tab-only retirement policy it cannot collapse36 states into3.
MODEL_CONFIG_LINEAGE='docs/development/model-config-surface-lineage-20261109.json'
MODEL_CONFIG_LINEAGE_SHA256='07fbda38b66368140ed88d5483ec3775ff43bd4e6ef5b77b167e7cd8910814dd'
for surface,section,description in [
    ('overlay:ModelConfigScreen:5fb902a57e','[data-service-catalog]','品牌/明确接口目录替代三协议添加入口；原七项适用义务继续由目录及编辑宿主承担。'),
    ('overlay:ModelConfigScreen:92d4a07590','[data-guided-service-page]','MC-12保存、显式选用、版本和在途归属由guided编辑页及统一选择器承担；不能由保存推定已应用。'),
    ('overlay:ModelConfigScreen:d6953cce06','[data-guided-service-page]','MC-08完整服务复制仍需交付；当前生产缺复制入口，原共享连接/名称/版本/失败/取消义务继续blocked。'),
]:
    s=SPECS[surface]
    s.update(preserve_retired_states=True,source_container='container',retired={
        'status':'retired_surface_with_retained_obligations','parent_surface':'settings:translation',
        'replacement_section':section,'replacement_description':description,
        'forbidden_production_selectors':[],'inventory_absence_required':True,
        'policy':'preserve_original_feature_states_add_legacy_entry_absence'})
    s['targets']={**s.get('targets',{}),'retired_surface_absent':section}
    s['gap']=description+' 原状态ID、适用性、两实际层与独立参照缺口不变；新增旧入口缺席负例不代替功能验收。'

def _new_surface(title,cases,components,paths,selector,steps=(),extras=(),present=None,gap=''):
    s=spec(title,cases,components,paths,'settings',selector,steps,present=present,extras=extras,
           needs_mapping=[*COMMON_STATES,*extras],gap=gap)
    s.update(retired=None,not_applicable=set(),responsibility_reason='逐项核对当前实际组件；异步、恢复、禁用、长文和交互义务独立保留，未有同态recipe的状态保持needs_mapping。')
    return s

_catalog_steps=[*settings_steps('translation'),click('[data-guided-section="saved"] [data-guided-action="catalog"]')]
_catalog=_new_surface('品牌与明确接口目录','FP-06 MC-01 MC-02 MC-03','C04 C05 C26 C27 C31','features/model-config/ServiceCatalogPicker.tsx features/model-config/ModelConfigScreen.tsx','#app-dialog',_catalog_steps,
    extras=['brand_choices','interface_choices','search_no_match','purpose_change','cancel'],present={'normal':[],'light':[],'brand_choices':[],'interface_choices':[click('[data-guided-brand="gemini"]')]},
    gap='guided-config.js175-178已有品牌/接口目录；生产busy、搜索与逐态交互需要独立同态映射、参照和真实执行。')
_catalog['targets']={'normal':'#app-dialog .guided-brands','light':'#app-dialog .guided-brands','brand_choices':'#app-dialog .guided-brands','interface_choices':'#app-dialog .guided-interfaces'}
_catalog['assertions']={name:[{'action':'assert_value','selector':'#guided-catalog-purpose','value':'translation'}] for name in ('normal','light','brand_choices')}
_catalog['assertions']['interface_choices']=[{'action':'assert_text_contains','selector':'#app-dialog','value':'选择Gemini的连接方式'}]
_catalog.update(not_applicable={'loading','error'},responsibility_reason='ServiceCatalogPicker仅对父组件已读取的catalog做同步用途/品牌/搜索筛选，无独立异步请求或错误分支；读取/失败由ModelConfigScreen宿主承担。空搜索结果、busy禁用、长文和取消由目录负责。')
SPECS['overlay:ServiceCatalogPicker:8a4aeed99b']=_catalog

_selector=_new_surface('统一已保存服务选择器','FP-06 MC-03 MC-12 MC-22','C04 C05 C26 C27 C31','features/model-config/ServiceSelector.tsx','#translation-service-picker',
    [click('#hud-window [data-service-action="open"]')],extras=['scope_global','scope_game','inheritance','eligibility_reason','saved_version','selection_failure','cancel_return_focus','grammar_purpose','context_word_purpose'],
    present={'normal':[],'light':[]},gap='service-picker.js114-140准确支持翻译选择器；生产精读/查词用途、异步失败与逐态行为不能继承翻译静态正常态。')
_selector['scenario']='reading'
_selector['assertions']={name:[{'action':'assert_text_contains','selector':'#translation-service-title','value':'选择翻译服务'}] for name in ('normal','light')}
SPECS['overlay:ServiceSelector:ddfdd2de73']=_selector
for purpose in ('translation','grammar','context_word'):
    s=_new_surface('添加服务用途 · '+purpose,'FP-06 MC-01 MC-02 MC-06 MC-22','C04 C26 C27','features/model-config/ModelConfigScreen.tsx features/model-config/ServiceCatalogPicker.tsx','#app-dialog',
        [*_catalog_steps,select('#guided-catalog-purpose',purpose)],present={'normal':[],'light':[]},
        gap='guided-config.js178明确列出三用途；仅正常/浅色目录选择有准确recipe，其余逐态义务保持待映射。')
    s['assertions']={name:[{'action':'assert_value','selector':'#guided-catalog-purpose','value':purpose}] for name in ('normal','light')}
    s.update(not_applicable={'loading','error'},responsibility_reason='catalogPurpose仅同步选择传入目录的initialPurpose，不独立读取或保存；catalog读取/失败归ModelConfigScreen宿主。三用途的空筛选、禁用、主题、窄窗、缩放和键盘仍分别保留。')
    SPECS['subpage:ModelConfigScreen:catalogPurpose:'+purpose]=s
for surface,title,paths in [
    ('overlay:ModelConfigScreen:3e3e52b7fc','服务编辑未保存确认','features/model-config/ModelConfigScreen.tsx'),
    ('overlay:SettingsWindow:ca82dd59c6','离开服务页未保存确认','windows/settings/SettingsWindow.tsx features/model-config/ModelConfigScreen.tsx'),
]:
    SPECS[surface]=_new_surface(title,'FP-06 MC-01 MC-06 MC-12','C05 C10 C26 C31',paths,'#app-dialog',settings_steps('translation'),extras=['continue_editing','discard_and_continue','secret_cleared'],
        gap='settings.js246的批准源含“保存并切换”；当前生产只有继续编辑/放弃，按钮与行为不等价，全部适用状态needs_mapping，不能标已有精确设计。')
    SPECS[surface].update(not_applicable={'empty','loading','error','disabled'},responsibility_reason='当前两个未保存Sheet均为同步继续/放弃确认，文本固定且按钮无pending/disabled/error分支；保存、配置读取与错误由编辑宿主负责。确认行为、秘密清理、长文、主题、窄窗、缩放和键盘仍是本Sheet义务。')
SPECS['overlay:ModelConfigScreen:6da213484c']=_new_surface('Google授权撤销确认','FP-06 MC-04 MC-11 MC-13','C05 C26 C30 C31','features/model-config/ModelConfigScreen.tsx features/model-config/useGoogleAuthorization.ts','#app-dialog',settings_steps('translation'),
    extras=['affected_services','revoke_pending','revoke_failed','remote_unconfirmed','cancel'],
    gap='当前批准整稿无Google撤销确认Sheet的精确状态；PRD AI-CFG-11要求新增授权/秘密输入界面补当前整稿并实机验证。原系统凭据撤销确认不能替代Google本地撤销/远端确认状态。')
for surface in ('overlay:ServiceCatalogPicker:8a4aeed99b','overlay:ServiceSelector:ddfdd2de73',
                *('subpage:ModelConfigScreen:catalogPurpose:'+purpose for purpose in ('translation','grammar','context_word'))):
    SPECS[surface]['assertions']['light'].append({'action':'assert_design_predicate','expression':"document.documentElement.dataset.theme === 'light'"})

# CaptureSheet's changed Sheet AST is a new production surface. The old
# code-owned identity keeps every original descriptor/layer and gains a separate
# absence obligation. Actual current surfaces never inherit its source recipes.
CAPTURE_SURFACE_LINEAGE = {
    'legacy_surface':'overlay:CaptureSheet:e1434a9e7d',
    'current_surface':'overlay:CaptureSheet:afa78bce55',
    'connecting_surface':'subpage:CaptureSheet:step:connecting',
    'source_path':PREFIX+'features/capture/CaptureSheet.tsx',
    'policy':'preserve_original_feature_states_add_legacy_entry_absence',
}
_legacy_capture=SPECS[CAPTURE_SURFACE_LINEAGE['legacy_surface']]
_legacy_capture.update(preserve_retired_states=True,source_container='container',retired={
    'status':'retired_surface_with_retained_obligations',
    'parent_surface':CAPTURE_SURFACE_LINEAGE['current_surface'],
    'replacement_section':'#app-dialog',
    'replacement_description':'当前选择/connecting/streams弹层承接功能义务；原22条状态的设计descriptor、ID和实际层完整保留，不等于新弹层拥有同态recipe。',
    'forbidden_production_selectors':[], 'inventory_absence_required':True,
    'policy':CAPTURE_SURFACE_LINEAGE['policy']})
_legacy_capture['targets']={**_legacy_capture.get('targets',{}),'retired_surface_absent':'#app-dialog'}
_legacy_capture['gap']='原CaptureSheet入口身份已退场；22条历史状态与source descriptor保留，新弹层及connecting真实状态需独立精确设计映射、参照与执行，均不据登记通过。'
_current_capture=spec('运行目标、接入进度与正文来源（当前AST）',
    'GL-11 GL-12 GL-13 GL-14 GL-15 GL-17 GL-19 GL-20','C05 C13 C14 C15 C35',
    'features/capture/CaptureSheet.tsx','existing-library','#app-dialog',[click('#library-running')],
    present={},extras=_legacy_capture['extras'],needs_mapping=[*COMMON_STATES,*_legacy_capture['extras']],
    gap='生产Sheet由select/connecting/streams和progressFailed分支组成，旧弹层正常态的recipe不能冒当前逐态参照；全部新状态needs_mapping，实际层not_run，产品blocked。')
_current_capture.update(retired=None,not_applicable=set(),responsibility_reason='真实选择、接入进度与选流由同一Sheet分支负责；其通用、视觉及交互状态独立保留，当前没有同态recipe。')
SPECS[CAPTURE_SURFACE_LINEAGE['current_surface']]=_current_capture
_connecting_extras=['verifying','preparing_resources','requesting_hook','waiting_handshake','waiting_text','candidates_ready','failed','superseded','close_preserves_operation','retry_explicit']
SPECS[CAPTURE_SURFACE_LINEAGE['connecting_surface']]=spec('自动接入阶段与显式失败重试',
    'GL-13 GL-14 GL-15 GL-17 GL-20','C13 C14 C15 C35','features/capture/CaptureSheet.tsx',
    'existing-library','#app-dialog',[click('#library-running')],present={},extras=_connecting_extras,
    needs_mapping=[*COMMON_STATES,*_connecting_extras],
    gap='真实step=connecting、八个AssociationStage与关闭保留操作/显式重试来自CaptureSheet；批准源无该完整同态回放，不能用静态选择页或JSON登记替代设计/真实executor。')

SPECS[CAPTURE_SURFACE_LINEAGE['connecting_surface']].update(retired=None,not_applicable=set(),responsibility_reason='真实connecting分支承担stage进度、等待、失败/失效、关闭和显式重试；通用视觉/交互压力义务不因当前缺recipe而删减。')

# The user-approved General V3 plus its explicitly approved pending/error states
# replace only this route's source recipes. The other discovered obligations,
# applicability, global AST checks and required execution layers are unchanged.
from current_general_visual_authority import STATE_NAMES as GENERAL_STATE_NAMES, PREFIX as GENERAL_SOURCE_PREFIX, design_recipe as general_design_recipe
_general=SPECS['settings:general']
_general.update(entry=GENERAL_SOURCE_PREFIX+'settings.html',selector='.window',steps=[],general_visual_source=True,
                present={name.replace('-','_'):[] for name in GENERAL_STATE_NAMES},needs_mapping=[],gap='独立General源/参照/真实组件/同包Native分别验收，受控视图不证明OS、存储或游戏。')
_general['targets']={name.replace('-','_'):'.window' for name in GENERAL_STATE_NAMES}
_general['assertions']={name.replace('-','_'):[] for name in GENERAL_STATE_NAMES}

# Imported by the existing current-design contract. No original obligation changes.
ALL_PAGE_STATES = tuple(_state_mapping(page,s,name) for page,s in SPECS.items() for name in _state_names(s) if _applicable(s,name))
STATE_IDS = frozenset(s['id'] for s in ALL_PAGE_STATES)

def _target(s,name): return s.get('targets',{}).get(name,s['selector'])

def _recipe(s,name):
    if s.get('general_visual_source'):
        # Translate the finite General source intent into the existing design
        # runner's typed action; no caller-supplied script capability is added.
        return [evaluate(step['script']) if step.get('action')=='evaluate' else step for step in general_design_recipe(name)]
    entry=s.get('entry',ENTRY)
    query={'scenario':s['scenario'],'theme':'dark',**s.get('query',{}),**s.get('state_queries',{}).get(name,{})}
    if name=='light': query['theme']='light'
    steps=[{'action':'navigate','path':entry,'query':query},*s['steps'],*s['present'].get(name,[])]
    if name=='narrow': steps.insert(0,{'action':'set_viewport','width':740 if entry==HISTORY_ENTRY else 1024,'height':1200 if entry==HISTORY_ENTRY else 768})
    steps.append(visible(_target(s,name)))
    steps+=s.get('assertions',{}).get(name,[])
    return steps

def _page_contract(root,page,s,production,fixtures):
    states=[]
    for name in _state_names(s):
        applicable=_applicable(s,name)
        present=name in s['present']
        gap=VERIFIED_DESIGN_GAPS.get((page,name))
        status='not_applicable' if not applicable else 'missing_design_state' if gap else 'existing_design_state' if present else 'unmapped_design_state'
        states.append({**_state_mapping(page,s,name),'required_layers':LAYERS.copy() if applicable else [],'state':name,'label':STATE_LABELS.get(name,name),'applicable':applicable,
            'design_status':status,
            'state_kind':'retirement_negative' if s.get('retired') and (not s.get('preserve_retired_states') or name=='retired_surface_absent') else 'visual_variant' if name in ('light','narrow','zoom_200','long_text') else 'interaction_or_content',
            'mapping_status':'not_applicable' if not applicable else 'mapped_recipe' if present else 'needs_mapping',
            'design_gap':gap['reason'] if gap else '',
            'gap_evidence':({**gap,'source_sha256':fixtures[gap['path']]} if gap else None),
            'mapping_gap':('尚未建立可核对的操作与状态断言；这不表示当前设计不存在该状态。' if applicable and not present and not gap else ''),
            'applicability_reason':s['responsibility_reason'],
            'variant_measurements':({'reference':'missing','same_data_fixture':'needs_mapping' if name in ('long_text','zoom_200') else 'recorded','geometry_clipping':'not_run'} if name in ('light','narrow','zoom_200','long_text') else None),
            'production_assertions':([{'action':'assert_absent','selector':v} for v in s['retired']['forbidden_production_selectors']] if s.get('retired') and (not s.get('preserve_retired_states') or name=='retired_surface_absent') else []),
            'design_recipe':_recipe(s,name),'target':{'selector':_target(s,name),'visibility':'required_visible','container':s['source_container'] if s.get('retired') else production['role']},
            'fixture':{'kind':'page_local_design_simulation','source_catalogue':'#/design_fixture_source_sha256','source_identity_sha256':identity(fixtures),'production_execution':'not_run'},
            'reference':{'status':'missing_independent_reference','path':None,'sha256':None},
            'execution':{layer:'not_run' for layer in LAYERS} if applicable else {},'product_status':'not_applicable' if not applicable else 'blocked'})
    result={'surface_id':page,'title':s['title'],'production':production,'owners':['UI','QA','PL'],'implementation_gap':s['gap'],'surface_status':'retired_surface_with_retained_obligations' if s.get('preserve_retired_states') else 'retired_production_surface' if s.get('retired') else 'active_production_surface','retirement':s.get('retired'),
        'inheritance':{'historical_catalogue':DESIGN+'/acceptance-v4/catalogue.json','policy':'lineage_only_requires_current_surface_state_and_dependency_review','current_reference_approval':None},'states':states}
    if s.get('general_visual_source'):
        from current_general_visual_authority import authority as general_authority,expected_mapping as general_mapping
        source=general_authority(root)
        result['source_supplement']=general_mapping(root)
        for row in result['states']:
            row['fixture']={'kind':'approved_general_source_simulation','source_catalogue':f'#/pages/{list(SPECS).index(page)}/source_supplement',
                            'source_identity_sha256':source['source_closure_sha256'],'production_execution':'not_run'}
    return result

def expected_contract(root):
    inv=inventory(root)
    found={p['id']:p for p in inv['pages']}
    retired={page for page,s in SPECS.items() if s.get('retired')}
    if set(found)-set(SPECS) or (set(SPECS)-retired)-set(found): raise ValueError('AST surface/mapping mismatch; unmapped='+str(sorted(set(found)-set(SPECS)))+' missing_active='+str(sorted((set(SPECS)-retired)-set(found))))
    recreated=[page for page in found if SPECS[page].get('retired') and SPECS[page]['retired'].get('inventory_absence_required')]
    if recreated: raise ValueError('legacy Sheet entry reintroduced; original obligations belong to current hosts: '+','.join(sorted(recreated)))
    fixtures={p:digest(_safe(root,p)) for p in DESIGN_FIXTURE_SHA256}
    if fixtures!=DESIGN_FIXTURE_SHA256: raise ValueError('design fixture source identity drift; do not refresh approved sources')
    pages=[]
    for page,s in SPECS.items():
        production=found.get(page)
        if production is None:
            parent=found[s['retired']['parent_surface']]
            production={**parent,'id':page,'kind':'retired_surface_obligation','route':page.rsplit(':',1)[-1]}
        production={**production,'currently_discovered':page in found}
        pages.append(_page_contract(root,page,s,production,fixtures))
    result={'schema_version':3,'target_id':TARGET,'composition_id':authority.CURRENT_COMPOSITION_ID,'surface_entries':authority.CURRENT_ROUTES,
            'source_approval':authority.current_source_approval(root),'authority_source_sha256':{authority.RECORD:authority.RECORD_SHA256,authority.CURRENT_RECORD:authority.CURRENT_RECORD_SHA256},'scope':'Current AST-discovered surfaces and explicit state obligations; new surfaces or states require review. This is not an exhaustive promise about future product behavior.',
            'inventory':{**{k:v for k,v in inv.items() if k!='pages'},'discovered_surface_ids':[p['id'] for p in inv['pages']]},'source_case_authorities':['docs/product/first-product-scope.md','docs/product/game-library-window-acceptance.csv','docs/product/reading-window-session-acceptance.csv','docs/product/model-configuration-acceptance.csv'],
            'design_fixture_source_sha256':fixtures,'reference_approval':'missing_independent_references','production_capture_adapter':None,'product_status':'blocked','pages':pages,
            'model_config_surface_lineage':{'path':MODEL_CONFIG_LINEAGE,'sha256':MODEL_CONFIG_LINEAGE_SHA256}}
    lineage_errors=validate_model_config_lineage(root,result)
    if lineage_errors: raise ValueError('; '.join(lineage_errors))
    return result

def model_config_lineage(root=ROOT):
    path=_safe(root,MODEL_CONFIG_LINEAGE)
    if digest(path)!=MODEL_CONFIG_LINEAGE_SHA256: raise ValueError('frozen model-config obligation lineage changed')
    return _read(path)

def validate_model_config_lineage(root,data):
    lineage=model_config_lineage(root)
    actual={row['id']:row for page in data['pages'] for row in page['states']}
    from current_reference_catalogue import descriptor
    errors=[]
    for original in lineage['original_all_page_states']:
        row=actual.get(original['id'])
        if row is None or any(row.get(key)!=value for key,value in original.items() if key not in ('surface_id','descriptor_sha256')) or identity(descriptor(row))!=original['descriptor_sha256']:
            errors.append('original state obligation/source descriptor changed: '+original['id'])
    current={row['id']:row for row in ALL_PAGE_STATES}
    for row in lineage['original_current_states']:
        if row['id'].startswith('all-page-') and current.get(row['id'])!=row:
            errors.append('original current809 mapping changed: '+row['id'])
    for original in lineage['legacy_surface_obligations']:
        page=next(p for p in data['pages'] if p['surface_id']==original['surface_id'])
        if ([row['id'] for row in page['states'][:-1]]!=original['original_state_ids']
                or page['states'][-1]['state']!='retired_surface_absent'
                or sum(row['applicable'] for row in page['states'][:-1])!=original['original_applicable_count']
                or page['production']['currently_discovered'] is not False):
            errors.append('retired Sheet must preserve original states and add actual AST absence: '+page['surface_id'])
    return errors

def expected_general_page(root=ROOT):
    # Finite route validation does not approve the unrelated global inventory.
    inv=inventory(root)
    found=[p for p in inv['pages'] if p['id']=='settings:general']
    if len(found)!=1 or found[0].get('role')!='settings' or found[0].get('route')!='general':
        raise ValueError('Exactly one actual settings:general AST route required')
    fixtures={p:digest(_safe(root,p)) for p in DESIGN_FIXTURE_SHA256}
    if fixtures!=DESIGN_FIXTURE_SHA256:raise ValueError('Original design fixture authority changed')
    return _page_contract(root,'settings:general',SPECS['settings:general'],{**found[0],'currently_discovered':True},fixtures)


def validate_general_mapping(root=ROOT,data=None):
    errors=[]
    try:
        data=_read(_safe(root,CONTRACT)) if data is None else data
        rows=[p for p in data.get('pages',[]) if p.get('surface_id')=='settings:general']
        if len(rows)!=1 or rows[0]!=expected_general_page(root):errors.append('General-only exact source/14-state/production dependency mapping differs')
        errors.extend(obligation_errors(data))
    except (OSError,ValueError,KeyError,TypeError,subprocess.SubprocessError) as error:
        errors.append('General-only permanent mapping invalid: '+str(error))
    return errors

def authoritative_case_ids(root):
    cases=set(re.findall(r'^\|\s*(FP-\d{2})\s*\|',_safe(root,'docs/product/first-product-scope.md').read_text(),re.M))
    for file in ('game-library-window-acceptance.csv','reading-window-session-acceptance.csv','model-configuration-acceptance.csv'):
        for row in csv.DictReader(_safe(root,'docs/product/'+file).read_text().splitlines()): cases.add(next(iter(row.values())))
    cases.update(re.findall(r'^###\s+(AI-CFG-\d{2})\s',_safe(root,'docs/product/model-configuration-prd.md').read_text(),re.M))
    return cases

def missing_design_state_ids(root=ROOT, required=None):
    missing=frozenset(_state_id(page,name) for (page,name) in VERIFIED_DESIGN_GAPS if _applicable(SPECS[page],name))
    return missing if required is None else missing.intersection(required)

def unmapped_design_state_ids(root=ROOT, required=None):
    result=frozenset(_state_id(page,name) for page,s in SPECS.items() for name in _state_names(s) if _applicable(s,name) and name not in s['present'] and (page,name) not in VERIFIED_DESIGN_GAPS)
    return result if required is None else result.intersection(required)

def unresolved_design_state_ids(root=ROOT, required=None):
    return missing_design_state_ids(root,required) | unmapped_design_state_ids(root,required)

def obligation_errors(data):
    pages=data.get('pages',[])
    ids=[s.get('id') for p in pages for s in p.get('states',[]) if s.get('applicable')]
    if len(ids)!=len(set(ids)) or set(ids)!=STATE_IDS: return ['missing/duplicate/unknown all-page state obligation']
    return []

def validate_contract(root=ROOT):
    errors=[]
    try:
        errors.extend(authority.current_user_approval_gaps(root))
        errors.extend(validate_general_mapping(root))
        expected=expected_contract(root)
        actual=_read(_safe(root,CONTRACT))
        errors.extend(obligation_errors(actual))
        if actual!=expected: errors.append('all-page exact obligation/recipe/dependency/reference mapping differs; missing state, hidden target, stale reference or fabricated approval forbidden')
        components={r['component_id'] for r in csv.DictReader(_safe(root,'docs/architecture/engineering-components.csv').read_text().splitlines())}
        cases=authoritative_case_ids(root)
        if len(STATE_IDS)!=len(ALL_PAGE_STATES): errors.append('duplicate all-page state ID')
        for gap in VERIFIED_DESIGN_GAPS.values():
            source=_safe(root,gap['path']).read_text()
            if not gap['reason'] or gap['anchor'] not in source or gap['quote'] not in source: errors.append('missing design claim requires actual source reason and anchor')
        for row in ALL_PAGE_STATES:
            if not set(row['source_case_ids'])<=cases or not set(row['component_ids'])<=components: errors.append('unknown authoritative all-page case/component: '+row['id'])
            for p in row['component_paths']:
                if not _safe(root,p).is_file(): errors.append('mapped production component missing: '+p)
        # Fail even if a caller attempted to approve pages while retaining the same
        # obligation set. No screenshot producer or raw-boolean bypass is installed.
        if actual.get('production_capture_adapter') is not None or actual.get('product_status')!='blocked' or actual.get('reference_approval')!='missing_independent_references': errors.append('all-page metadata cannot grant product or reference acceptance')
    except (OSError,ValueError,KeyError,TypeError,subprocess.SubprocessError) as e: errors.append('all-page contract invalid: '+str(e))
    return errors

def _native_product_path(path):
    return (path.startswith(('native/','crates/','apps/desktop/src-tauri/'))
            or bool(re.search(r'\.(?:rs|c|cc|cpp|cxx|h|hh|hpp|hxx|inc|inl|ipp|m|mm|swift)$',path,re.I)))

def _nonproduction_path(path):
    """Naming only exempts an unreferenced test/document/tooling path.

    Production import closures are matched before this predicate. A fixture or
    documentation asset imported by the product therefore keeps its obligations.
    """
    parts=path.split('/')
    name=parts[-1]
    # Only explicit repository documentation/quality roots have an independent
    # nonproduction role. A doc directory nested inside a product root does not.
    if path in QUALITY_ONLY_PATHS or path.startswith(('docs/','openspec/','scripts/quality/')):
        return True
    # The discovered graph covers TypeScript entry points, not native compilation
    # units. Native paths and includes cannot use doc/test names as an exemption.
    if _native_product_path(path):
        return False
    test_path=(bool(set(parts[:-1]) & {'tests','test','__tests__','__mocks__'})
               or bool(re.search(r'(?:^test_|(?:\.|_)test\.|(?:\.|_)spec\.)',name)))
    # Naming narrows only UI tests for which the real TS entry closure was
    # already checked by affected_state_ids, never other product resource roots.
    ui_test=(path.startswith('apps/desktop/tests/')
             or path.startswith('apps/desktop/src/') and test_path
             and bool(re.search(r'\.[cm]?[jt]sx?$',name)))
    root_document=(len(parts)==1 and
                   (name.endswith(('.md','.mdx','.rst'))
                    or name.lower() in {'readme','license','notice','copying','authors','changelog'}))
    return ui_test or root_document

def _potential_product_path(path):
    # Unknown roots, binary resources and build scripts have no proven exclusion
    # from the product. Only the explicit nonproduction classes may be omitted.
    return not _nonproduction_path(path)

def affected_state_ids(root, paths):
    """Actual TS import/CSS closures. Unknown relevant impact conservatively means all."""
    if paths is None: return STATE_IDS
    if not isinstance(paths,(tuple,list,set,frozenset)) or any(not isinstance(p,str) for p in paths): raise ValueError('actual changed paths required')
    inv=inventory(root,production_dependencies=True); by_page={p['id']:set(p['dependency_paths']) for p in inv['pages']}; result=set()
    production_dependencies=set(inv['production_dependency_paths'])
    # Absent retired surfaces remain negative obligations of their actual parent.
    # Match expected_contract's parent projection instead of dropping their states
    # merely because the approved legacy navigation is no longer in the AST.
    for page,s in SPECS.items():
        if not s.get('retired') or page in by_page: continue
        parent=s['retired']['parent_surface']
        if parent not in by_page: return STATE_IDS
        by_page[page]=set(by_page[parent])
    for p in paths:
        _safe(root,p)
        matching={page for page,deps in by_page.items() if p in deps}
        if not matching:
            if p in production_dependencies or _potential_product_path(p): return STATE_IDS
            continue
        for page in matching:
            if page not in SPECS: return STATE_IDS
            result.update(_state_id(page,n) for n in _state_names(SPECS[page]) if _applicable(SPECS[page],n))
    return frozenset(result)

def summary(root=ROOT):
    data=_read(_safe(root,CONTRACT)); rows=[s for p in data['pages'] for s in p['states']]
    errors=validate_contract(root)
    missing=sum(s['design_status']=='missing_design_state' for s in rows)
    unmapped=sum(s['design_status']=='unmapped_design_state' for s in rows)
    gaps=list(errors)
    if missing: gaps.append('current page design states are missing: '+str(missing))
    if unmapped: gaps.append('current page design states need reviewed executable mapping: '+str(unmapped))
    from current_reference_catalogue import assess as reference_assessment
    from current_design_alignment import registered_execution_checks
    from current_preview_authority import source_mapping_gaps
    required = {s['id'] for s in rows if s['applicable']}
    references = reference_assessment(root, required)
    checks, capability_gaps = registered_execution_checks(required)
    gaps += references['errors'] + references['gaps'] + capability_gaps
    gaps += source_mapping_gaps(root, rows, required)
    # Registration and source references are not actual production receipts.
    # This read-only inventory does not accept a producer's self-reported pass.
    gaps.append('current-target per-state real React and packaged-native accepted execution receipts are required')
    return {'schema_version':1,'target_id':TARGET,'composition_id':authority.CURRENT_COMPOSITION_ID,
            'status':'blocked' if gaps else 'passed','metadata_errors':errors,'release_gaps':gaps,
            'product_tests_executed':False,'product_alignment_approved':False,
            'discovered_surface_count':len(data['inventory']['discovered_surface_ids']),
            'applicable_state_count':sum(s['applicable'] for s in rows),'missing_design_state_count':sum(s['design_status']=='missing_design_state' for s in rows),
            'unmapped_design_state_count':sum(s['design_status']=='unmapped_design_state' for s in rows),'retired_production_surface_count':sum(p['surface_status'] in ('retired_production_surface','retired_surface_with_retained_obligations') for p in data['pages']),'visual_variant_count':sum(s['state_kind']=='visual_variant' and s['applicable'] for s in rows),'existing_design_state_count':sum(s['design_status']=='existing_design_state' for s in rows),'not_applicable_state_count':sum(not s['applicable'] for s in rows),
            'independent_references':('invalid' if references['errors'] else 'complete' if not references['gaps'] else 'partial' if references['approved_state_ids'] else 'missing'),
            'approved_reference_state_count':len(set(references['approved_state_ids']) & required),
            'registered_current_checks':sorted(checks),'production_execution':'requires_actual_receipts',
            'note':'Counts describe this actual inventory snapshot, never future completeness or product acceptance.'}

def assessment_errors(root, report, exit_code):
    """Bind typed read-only facts and exit status to this source's actual gaps."""
    if not isinstance(report,dict): return ['all-page assessment must be an object']
    errors=[]
    counts=('schema_version','discovered_surface_count','applicable_state_count',
            'missing_design_state_count','unmapped_design_state_count',
            'retired_production_surface_count','visual_variant_count',
            'existing_design_state_count','not_applicable_state_count','approved_reference_state_count')
    if (any(type(report.get(key)) is not int or report[key]<0 for key in counts)
            or report.get('schema_version')!=1
            or any(not isinstance(report.get(key),list)
                   or any(not isinstance(value,str) for value in report[key])
                   for key in ('metadata_errors','release_gaps'))
            or report.get('product_tests_executed') is not False
            or report.get('product_alignment_approved') is not False):
        errors.append('all-page assessment requires typed counts, gap lists and read-only product facts')
    if (not isinstance(report.get('status'),str) or report.get('status') not in {'passed','blocked'} or type(exit_code) is not int
            or exit_code not in (0,1)
            or (exit_code==0)!=(report.get('status')=='passed')
            or isinstance(report.get('release_gaps'),list)
            and bool(report['release_gaps'])!=(report.get('status')=='blocked')):
        errors.append('all-page assessment exit/status must match actual release gaps')
    if report!=summary(root):
        errors.append('all-page assessment must match independently recomputed current inventory and actual gaps')
    if report.get('metadata_errors')!=[]:
        errors.append('all-page assessment metadata contract is invalid')
    return errors

def write_contract(root):
    data=expected_contract(root)
    lineage=model_config_lineage(root)
    original_applicable={s['id'] for s in lineage['original_all_page_states'] if s['applicable']}
    added=len(STATE_IDS-original_applicable)
    current_count=len(lineage['original_current_states'])+added
    _safe(root,CONTRACT).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
    lines=['# 当前完整页面与状态闭包','',f'目标：`{TARGET}`；完整组合：`{authority.CURRENT_COMPOSITION_ID}`。本表来自当前 TypeScript AST 实际目录及代码权威状态义务。新页面、弹层、共享组件和新状态必须补映射。','',
           '完整设计源由[完整批准](design-approval-complete-20261031.json)绑定原57源与R79历史4源；C35以[已采用五项组件补充](design-approval-preview-20261104.json)为新目标，比较画布不替代完整主窗。CSS wallpaper另作依赖。原4项有限真实React检查和General14状态的React/同包Native执行分别限定能力，其他状态仍缺；当前执行选择必须从已注册adapter.states逐层求覆盖。设计确认、独立参照、真实组件与同包原生分别留证；原29状态、原PNG/token/CSV和全部义务保留，不能由登记或有限能力宣布产品通过。','',
           '每条状态分别记录入口query、设计操作recipe、target、固定fixture源身份、需求/组件/Owner和实际层。`existing_design_state`已登记可回放recipe与具体状态断言；是否实际执行以独立运行回执为准，仍不代表参照或产品通过；`unmapped_design_state` / `needs_mapping`只表示尚未完成可核对映射，不能推断设计不存在；只有逐项理由和固定源码anchor支持的状态才标`missing_design_state`。纯展示组件按实际props标not_applicable；明暗/窄窗/200%/长文另列视觉变体与未执行的参照/数据/几何测量。','',
           '## 实际页面矩阵','', '| 实际surface | 生命周期 | 页面/弹层/组件 | 适用义务 | 已映射 | 待映射 | 已核实设计缺项 | 实现差距 |','|---|---|---|---:|---:|---:|---:|---|']
    for p in data['pages']:
        count=sum(s['applicable'] for s in p['states']); missing=sum(s['design_status']=='missing_design_state' for s in p['states']); mapped=sum(s['design_status']=='existing_design_state' for s in p['states']); unmapped=sum(s['design_status']=='unmapped_design_state' for s in p['states'])
        lines.append(f"| `{p['surface_id']}` | {p['surface_status']} | {p['title']} | {count} | {mapped} | {unmapped} | {missing} | {p['implementation_gap'] or '需当前逐状态参照与实际执行'} |")
    lines+=['','设计recipe的evaluate_design_api明确按invoke_function_body执行脚本体，不能返回未调用的函数；异步操作需等待。每个已有状态另有可见控件/属性/状态断言。入口query在代码中逐surface/状态合并一次，浅色不会二次navigate丢失historyState；历史窄窗740×1200，200%使用实际CSS zoom=2。recipe JSON合同通过只证明登记，真实回放、几何/裁切和产品执行另留证。','', '旧ModelConfig六实体页签与Reading三数据页签标为retired_production_surface：只保留旧导航不得存在的负例与当前guided服务编辑/用途高级设置、同页折叠数据集映射。它们可以从实际AST消失，不要求重新设计或复原旧页面；terms具体编辑位置尚待映射，不能用旧术语表tab替代。','',
            f'旧ModelConfig添加/应用/复制三个Sheet采用独立的retired_surface_with_retained_obligations策略：[冻结迁移身份](model-config-surface-lineage-20261109.json)绑定原856条记录、780项all-page适用义务与完整809状态定义。原三个Sheet的39条记录（36项适用）、全部ID/适用性/required layers/blocked状态及原设计descriptor逐项保留，另加3项AST缺席负例；六实体tab的旧快捷退役规则不适用。MC-12保存、显式选用、版本和在途归属继续由当前编辑页/选择器承担；MC-08完整复制仍无生产入口，不能由复制旧Sheet退役抹掉。后续实际入口按精确身份差集累计追加{added}项（包含原92项与本轮CaptureSheet增量）；新同步目录/确认的16条不适用记录仍保留并逐条说明宿主职责。全局{current_count}适用状态不表示产品通过。','',
            'CaptureSheet保留旧e143入口的22条状态与完整source descriptor，追加独立AST缺席；实际afa78当前Sheet及connecting子页不沿用旧recipe授予映射，全量needs_mapping/blocked，两层保持not_run。', '', '品牌/明确接口目录与三个用途入口仅采用guided-config.js175-178的准确recipe；统一选择器仅对service-picker.js114-140的翻译正常/浅色态建立映射。未保存确认与settings.js246“保存并切换”的按钮/行为不同，Google撤销没有当前批准的精确Sheet，均保持needs_mapping；新增授权界面仍须按PRD AI-CFG-11补整稿并实机验证。','', '## 继承与实际来源','',
            '`acceptance-v4/catalogue.json`仅保留历史血缘；旧参照不能覆盖当前完整组合。默认入口仍为guided；`main:history`唯一使用R79历史入口与hx DOM。历史20项中17项已有recipe，独立详情加载/错误及revision冲突3项保持待映射，不以列表加载或保存失败冒充。原20261019的16源中app/data/game-library/index/review-gaps/settings已有变化，页面专属CSS未变不证明整个依赖闭包未变。','',
            '当前`scenario=reading`默认只开game/HUD，主会话必须先打开主窗并进入session；`scenario=hud`不是独立分支，抽象`reading settings hud`不是可执行URL。设置子页通过实际分类按钮进入。独立`surface=word-card`不经过WindowRoot四窗口，需要独立参照和同包原生内容撑窗验证。','',
            '## 漏项与影响拒绝规则','',
            'AST读取MainPage/SettingsPage union与真实导航双向比对、WindowRoot实际分支、bootstrap临时surface、Sheet/ActionMenu实例以及共享UI导出。新增或删除入口不匹配时拒绝。每页闭包递归解析本地TS/TSX barrel imports和CSS依赖；共享Select/Controls/token影响所有实际使用页；无法定位的产品或native影响保守要求全部状态。','',
            '检查逐状态对比代码权威义务及source/recipe/target/reference/execution身份；删行、漏状态、旧参考、隐藏target元数据、伪approved、跳过真实产品层均拒绝。合同检查通过只证明完整登记与诚实阻断，不能代替截图、真实交互或发布验收。','',
            '```sh','python3 scripts/quality/all_page_design_coverage.py --check --json','python3 scripts/quality/all_page_design_coverage.py --assess --json','node scripts/quality/page-inventory.mjs --root .','python3 -m unittest discover -s scripts/quality -p test_all_page_design_coverage.py -v','```','']
    _safe(root,DOCUMENT).write_text('\n'.join(lines))

if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--root',type=Path,default=ROOT);ap.add_argument('--check',action='store_true');ap.add_argument('--assess',action='store_true');ap.add_argument('--write-contract',action='store_true');ap.add_argument('--json',action='store_true');opts=ap.parse_args()
    if opts.write_contract: write_contract(opts.root)
    errors=validate_contract(opts.root); report=summary(opts.root) if opts.assess else {'status':'passed' if not errors else 'failed','errors':errors,'scope':'metadata_only_product_blocked'}
    print(json.dumps(report,ensure_ascii=False,indent=2) if opts.json else report)
    raise SystemExit(1 if errors or opts.assess and report['status']!='passed' else 0)
