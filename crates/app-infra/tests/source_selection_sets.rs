//! Actual schema5 Repository/SQLite tests. Controlled Hook facts are not live-game acceptance.
#[allow(dead_code)]
#[path = "storage_acceptance_support/frozen_hosts.rs"]
mod frozen_hosts;
use app_core::{
    application::binding_journey::build_selection_match_snapshot,
    contracts::*, domain::{capture::*, canonical_hash, content_hash},
    ports::{ProductRepository, runtime::SourceSelectionCommit},
};
// Compile the exact product storage module as a private cfg(test) host. This
// gives the parent binary real production-boundary observations without adding
// any public/debug fault selector to the non-test app-infra library. The child
// fault executor below is separately compiled from app-infra's actual lib target.
#[path = "../src/storage/mod.rs"]
mod storage;
pub use app_infra::{offline_translation, providers};
use storage::SqliteStore;
use rusqlite::{Connection as Sqlite, params};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::{Path,PathBuf}, process::Command as Process};

fn directory(label: &str) -> PathBuf {
    let base = std::env::var_os("LUNA_BINDING_OUTPUT").map(PathBuf::from).unwrap_or_else(||root().join(".build/source-selection-test-artifacts"));
    let path = base.join(format!("{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}
fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap() }
fn game() -> GameInput {
    GameInput { game_id: None,row_revision:None,display_name:"controlled selection game".into(),original_title:None,
        source_language:Some("ja".into()),cover_asset_ref:None,target:GameTarget {
            environment_kind:"crossover".into(),environment_key:"crossover:/controlled/selection".into(),bottle_locator:Some("/controlled/selection".into()),
            launch_path:Some("C:\\selection.exe".into()),canonical_exe_path:Some("C:\\selection.exe".into()),target_path_key:None,
            working_directory:None,arguments:vec![],architecture:Some("x86".into()),verification_state:"verified".into(),
        } }
}
fn command(store: &SqliteStore, operation: &str, command: Command, now: i64) -> CommandReceipt {
    store.dispatch(CommandEnvelope {operation_id:operation.into(),expected_revision:None,command},now).unwrap()
}
struct Fixture { store:SqliteStore,path:PathBuf,session:HookSessionInput,sequence:u64 }
impl Fixture {
    fn new(label: &str, owned: bool) -> Self {
        let path = directory(label).join("profile.sqlite3");
        let store = SqliteStore::open(&path,"selection-profile","selection-server",1).unwrap();
        let game_id = owned.then(|| command(&store,"save-game",Command::SaveGame(Box::new(game())),2).result_id.unwrap());
        let session = HookSessionInput {session_id:"selection-session".into(),game_id,environment_key:"crossover:/controlled/selection".into(),target_exe_path:"C:\\selection.exe".into(),windows_pid:100,
            process_created_u64:"18446744073709551615".into(),architecture:"x86".into(),broker_instance_id:"selection-broker".into(),
            resource:ResourceSnapshotInput { snapshot_id:"selection-core".into(),kind:"core".into(),version:"controlled-v1".into(),manifest_sha256:content_hash("selection-core"),contract_version:"abi1-jsonl2".into(),metadata_json:"{}".into() },
            target_identity_sha256:content_hash("full controlled target identity"),};
        store.open_hook_session(session.clone(),3).unwrap();
        let mut fixture = Self {store,path,session,sequence:0};
        fixture.text("source-a","候補A",4);
        fixture.text("source-b","候補B",5);
        fixture
    }
    fn text(&mut self, source: &str, text: &str, now:i64) -> CaptureOutcome {
        self.sequence += 1;
        self.store.ingest_hook_text(HookTextInput {session_id:self.session.session_id.clone(),event_id:format!("event-{}",self.sequence),sequence_u64:self.sequence.to_string(),
            source:HookSourceInput {source_id:source.into(),host_instance_id:self.session.broker_instance_id.clone(),hook_address_u64:"18446744073709551615".into(),context_u64:"9007199254740993".into(),subcontext_u64:"1".into(),hook_code:format!("code-{source}"),hook_name:"same Hook name".into(),encoding:"unicode".into(),stream_identity_sha256:content_hash(source),identity_extensions_json:"{}".into()},
            text:text.into(),source_language:"ja".into()},now).unwrap()
    }
    fn projection(&self) -> SourceSelectionProjection {
        self.store.source_selection(SourceSelectionQuery {session_id:self.session.session_id.clone(),expected_profile_generation:1}).unwrap()
    }
    fn input(&self, operation:&str, sources:&[&str], remember:bool) -> SourceSelectionCommit {
        let projection = self.projection();
        let request = BeginReadingRequest {operation_id:operation.into(),session_id:self.session.session_id.clone(),expected_profile_generation:1,
            expected_set_revision:projection.current.map(|set|set.set_revision),source_ids:sources.iter().map(|s|s.to_string()).collect(),remember};
        let selected = request.source_ids.iter().filter_map(|id| projection.sources.iter().find(|source|&source.source_id==id).cloned()).collect::<Vec<_>>();
        let match_snapshot_json = build_selection_match_snapshot(remember,&self.session.target_identity_sha256,&selected).unwrap();
        SourceSelectionCommit {request_sha256:canonical_hash(&request).unwrap(),request,profile:projection.profile,active_session:self.session.clone(),match_snapshot_json}
    }
    fn commit(&self, operation:&str, sources:&[&str], remember:bool, now:i64) -> SelectionCommitReceipt {
        self.store.commit_source_selection(self.input(operation,sources,remember),now).unwrap()
    }
    fn scalar(&self, sql:&str) -> i64 { Sqlite::open_with_flags(&self.path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap().query_row(sql,[],|r|r.get(0)).unwrap() }
    fn backup(&self,name:&str) -> PathBuf {
        let path=self.path.parent().unwrap().join(name);
        self.store.backup(&path).unwrap(); path
    }
}

fn configured(store:&SqliteStore) {
    let config=Configuration {revision_id:String::new(),connections:vec![Connection {connection_id:"connection".into(),display_name:"controlled local".into(),protocol:"openai_compatible".into(),deployment:"local".into(),auth_mode:"none".into(),endpoint:Some("http://127.0.0.1:1234/v1".into()),credential_pool_id:None,credential_pool_revision:None,proxy_mode:"disabled".into(),network_metadata:BTreeMap::new(),availability_intent:true,adapter_binding:None,preset_source:None}],
        models:vec![Model {default_templates:BTreeMap::new(),model_id:"model".into(),connection_id:"connection".into(),actual_model_id:Some("controlled".into()),display_name:"controlled".into(),selection_source:"manual".into(),capabilities:BTreeMap::new(),parameters:BTreeMap::new()}],
        bindings:vec![PurposeBinding {binding_id:"translation".into(),scope_key:"global".into(),game_id:None,purpose:"translation".into(),binding_mode:"override".into(),enabled:true,output_mode:Some("text".into()),source_language:Some("ja".into()),target_language:Some("zh-Hans".into()),template_id:None,context:BTreeMap::new(),schedule:BTreeMap::new(),parameters:BTreeMap::new(),engines:vec![Engine {engine_id:"engine".into(),model_id:"model".into(),enabled:true,template_id:None,parameters:BTreeMap::new()}]}],templates:vec![],term_sets:vec![],codec_version:None};
    let saved=command(store,"save-config",Command::SaveConfiguration {configuration:config,expected_head_revision:0},10);
    command(store,"apply-config",Command::ApplyConfiguration {revision_id:saved.result_id.unwrap(),expected_head_revision:0},11);
}

#[test]
fn bj_08_storage() {
    let mut f=Fixture::new("bj08",true);
    let before=f.scalar("SELECT count(*) FROM source_selection_sets");
    let input=f.input("begin-two", &["source-a","source-b"],false);
    assert_eq!(f.scalar("SELECT count(*) FROM source_selection_sets"),before,"a draft does not commit");
    let receipt=f.store.commit_source_selection(input.clone(),20).unwrap();
    assert_eq!(receipt.selection.members.len(),2);
    assert_ne!(receipt.selection.members[0].selection_revision,receipt.selection.members[1].selection_revision);
    let replay=f.store.commit_source_selection(input.clone(),21).unwrap();
    assert!(replay.replayed);
    assert_eq!(receipt.selection,replay.selection);
    assert_eq!(f.scalar("SELECT count(*) FROM source_selection_sets"),1);
    let mut conflicting=input;
    conflicting.request.source_ids.reverse(); conflicting.request_sha256=canonical_hash(&conflicting.request).unwrap();
    assert_eq!(f.store.commit_source_selection(conflicting,22).unwrap_err().code,"operation_conflict");
    let a=f.text("source-a","甲。",1000).visible_version.unwrap();
    let b=f.text("source-b","乙。",3000).visible_version.unwrap();
    assert_ne!(a.unit_id,b.unit_id,"different sources cannot merge");
    assert_eq!(f.store.unit(a.unit_id,1).unwrap().source_text,"甲。");
    assert_eq!(f.store.unit(b.unit_id,1).unwrap().source_text,"乙。");
    assert_eq!(f.scalar("SELECT count(*) FROM current_source_selection_members"),2);
    let selection_snapshot=f.backup("multi-member.sqlite3");
    if std::env::var_os("LUNA_BINDING_NONCE").is_some() {write_selection_artifact(&selection_snapshot);}
}

#[test]
fn bj_09_storage() {
    let mut f=Fixture::new("bj09-owned",true);
    let saved=f.commit("remember",&["source-a","source-b"],true,20);
    let saved_raw=f.projection().saved.unwrap().match_snapshot_json;
    let temporary=f.commit("temporary",&["source-b"],false,21);
    assert_eq!(temporary.selection.persistence,SelectionPersistence::Temporary);
    let projection=f.projection();
    assert_eq!(projection.saved.as_ref().unwrap().set,saved.selection);
    assert_eq!(projection.saved.as_ref().unwrap().match_snapshot_json,saved_raw);
    assert_eq!(projection.saved.unwrap().state,SourceSelectionState::RequiresReview);
    assert_eq!(projection.sources[0].address.as_deref(),Some("18446744073709551615"));
    assert_eq!(projection.sources[0].context.as_deref(),Some("9007199254740993"));
    let unowned=Fixture::new("bj09-unowned",false);
    assert!(!unowned.projection().remember_allowed);
    assert_eq!(unowned.store.commit_source_selection(unowned.input("cannot-remember",&["source-a"],true),22).unwrap_err().code,"remember_requires_game");
    assert_eq!(unowned.scalar("SELECT count(*) FROM source_selection_sets"),0);
    let mut fake=f.input("fake-proof",&["source-a"],true);
    fake.match_snapshot_json="{\"schemaVersion\":1,\"portable\":true}".into();
    assert_eq!(f.store.commit_source_selection(fake,23).unwrap_err().code,"invalid_saved_selection");
    command(&f.store,"stop-for-retention",Command::Disconnect,30);
    let cleanup=f.store.maintain_storage(app_core::domain::retention::RetentionPolicy {max_rows_per_batch:100,raw_payload_before_ms:Some(10),..Default::default()},Default::default(),"payload-cleanup".into(),31).unwrap();
    assert_eq!(cleanup.removed_raw_payloads,2);
    assert_eq!(f.scalar("SELECT count(*) FROM source_selection_sets WHERE persistence_kind='saved_snapshot'"),1);
    assert_eq!(f.scalar("SELECT count(*) FROM source_selection_members"),3);
    f.session.session_id="restarted-session".into();
    f.session.broker_instance_id="restarted-broker".into();
    f.session.windows_pid=101;
    f.session.process_created_u64="18446744073709551614".into();
    f.session.target_identity_sha256=content_hash("new full controlled process identity");
    f.store.open_hook_session(f.session.clone(),40).unwrap();
    f.text("new-source-a","新候補",41);
    let restart=f.projection();
    assert_eq!(restart.state,SourceSelectionState::RequiresReview);
    assert!(restart.current.is_none(),"a saved session descriptor must not automatically reselect ASLR/session sources");
    assert_eq!(restart.saved.as_ref().unwrap().match_snapshot_json,saved_raw);
    assert!(restart.sources.iter().all(|source|source.source_id=="new-source-a"));
}

fn inspect(path:&Path, version:u32) {
    // All callers own a closed test database. Deliver a standalone artifact
    // before immutable inspection; never discard an uncheckpointed live WAL.
    let closed=Sqlite::open(path).unwrap();
    let checkpoint:(i64,i64,i64)=closed.query_row("PRAGMA wal_checkpoint(TRUNCATE)",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert_eq!(checkpoint.0,0);
    let mode:String=closed.query_row("PRAGMA journal_mode=DELETE",[],|row|row.get(0)).unwrap();
    assert_eq!(mode,"delete");
    drop(closed);
    let script="import sys;from pathlib import Path;sys.path.insert(0,sys.argv[1]+'/scripts/quality');import storage_schema as s;version=int(sys.argv[3]);contract='docs/architecture/database/'+('schema-contract.json' if version==1 else 'v'+str(version)+'/schema-contract.json');e=s.compare_schema_file(Path(sys.argv[1]),Path(sys.argv[2]),'profile',contract);print(e);sys.exit(bool(e))";
    let output=Process::new("python3").args(["-c",script]).arg(root()).arg(path).arg(version.to_string()).output().unwrap();
    assert!(output.status.success(),"{} {}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
}

fn seed_historical_lineage(path:&Path) {
    let db=Sqlite::open(path).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    db.execute("INSERT INTO games(game_id,display_name,row_revision,created_at_ms,updated_at_ms) VALUES('legacy-game','old game',1,1,1)",[]).unwrap();
    db.execute("INSERT INTO game_targets(target_id,game_id,environment_kind,environment_key,bottle_locator,canonical_exe_path,target_path_key,architecture,verification_state,row_revision,updated_at_ms) VALUES('legacy-target','legacy-game','crossover','crossover:/controlled/selection','/controlled/selection','C:\\selection.exe','c:/selection.exe','x86','verified',1,1)",[]).unwrap();
    db.execute("INSERT INTO resource_snapshots VALUES('legacy-core','core','old',?1,'1','{}',1)",[content_hash("old core")]).unwrap();
    db.execute("INSERT INTO sessions VALUES('legacy-session','legacy-game','hook','stopped','crossover:/controlled/selection','C:\\selection.exe',1,'1','x86','legacy-broker','legacy-core',?1,1,2)",[content_hash("old full identity")]).unwrap();
    db.execute("INSERT INTO capture_sources VALUES('legacy-source','legacy-session','legacy-broker','18446744073709551615','9007199254740993','1','old-code','old-hook','utf8',?1,'{}',1)",[content_hash("old source identity")]).unwrap();
    db.execute("INSERT INTO source_selections VALUES('legacy-session',1,'legacy-source','saved_snapshot','{\"schemaVersion\":999,\"unknown\":\"原字节\"}',2)",[]).unwrap();
    let text="旧来の原文。";
    db.execute("INSERT INTO raw_events VALUES('legacy-event','legacy-session','legacy-source','1',1,'text',?1,?2,2,'intact')",params![text.len() as i64,content_hash(text)]).unwrap();
    db.execute("INSERT INTO raw_event_payloads VALUES('legacy-event',?1,2)",[text.as_bytes()]).unwrap();
    db.execute("INSERT INTO reading_units VALUES('legacy-unit','legacy-session',1,'finalized',2)",[]).unwrap();
    db.execute("INSERT INTO reading_unit_revisions VALUES('legacy-unit',1,'legacy-session',?1,?2,?3,'ja','original-v1','{\"preserved\":true}',1,2)",params![text,text.len() as i64,content_hash(text)]).unwrap();
    db.execute("INSERT INTO source_refs VALUES('legacy-unit',1,'legacy-session',0,'legacy-event',0,?1)",[text.len() as i64]).unwrap();
}

#[test]
fn bj_11_storage() {
    if let Some(path)=std::env::var_os("LUNA_STORAGE_SCHEMA5_KILL_DATABASE") {kill_child_test_host(&PathBuf::from(path));return;}
    let out=directory("bj11");
    let mut runtime=RuntimeCapture::requested();
    let mut parent_parts=serde_json::Map::new();
    let fresh=out.join("fresh.sqlite3");
    let fresh_observation=runtime.as_ref().map(|_|SqliteStore::observe_schema5_fresh_open(&fresh,"bj_11_storage","migration-profile","new"));
    if let Some(r)=&mut runtime {r.event("fresh","writer_open_started",json!({"repository":"SqliteStore::open","now_ms":100,"server_id":"new"}));}
    drop(SqliteStore::open(&fresh,"migration-profile","new",100).unwrap());
    if let Some(guard)=&fresh_observation {guard.control.emit("caller_received_success","caller",json!({}));}
    inspect(&fresh,5);
    if let Some(r)=&mut runtime {
        let source_identity_before=file_identity(&fresh);r.snapshot("fresh-after",&fresh);let source_identity_after=file_identity(&fresh);assert_eq!(source_identity_before,source_identity_after);let capture=json!({"source_path":fresh,"source_identity_before":source_identity_before,"source_identity_after":source_identity_after,"snapshot_base":r.base,"snapshot":r.snapshots["fresh-after"],"captured_at_ms":wall_ms()});
        r.route("fresh",None,"fresh-after",5,None,None);let db=Sqlite::open(&fresh).unwrap();r.sqlite=db.query_row("SELECT sqlite_version(),sqlite_source_id()",[],|row|Ok(json!({"version":row.get::<_,String>(0)?,"source_id":row.get::<_,String>(1)?}))).unwrap();drop(db);
        let guard=fresh_observation.as_ref().unwrap();guard.control.emit("independent_current5_db_ledger_invariants","independent_readback",json!({"before":null,"after":r.snapshots["fresh-after"],"backup":null,"raw_provenance_policy":"original callback facts plus post-open first identity observation; final identity added only as invocation artifact metadata"}));
        let proof=guard.seal_fresh(capture);write_new(&r.base.join("fresh-raw-closure-reference.json"),&json!({"kind":"actual-fresh-raw-closure-reference/v1","proof":root_artifact(&out.join("fresh-raw-closure.json")),"invocation_id":proof["binding"]["invocation_id"]}));parent_parts.insert("fresh".into(),json!({"binding":guard.control.binding(),"timeline":guard.control.timeline()}));r.event("fresh","writer_open_committed",json!({"schema_version":5}));
    }
    drop(fresh_observation);
    for version in 1..=4 {
        let path=out.join(format!("from-{version}.sqlite3"));
        if version==1 {
            let db=Sqlite::open(&path).unwrap();
            db.execute_batch(include_str!("../src/storage/migrations/001_profile.sql")).unwrap();
            db.execute_batch(include_str!("../src/storage/migrations/001_config.sql")).unwrap();
            db.execute("INSERT INTO schema_migrations VALUES(1,?1,0,'controlled-old-one',0)",[app_infra::storage::profile_migration_sha256()]).unwrap();
            db.execute("INSERT INTO profile_meta VALUES(1,'migration-profile',0,1,'old','active',0,0)",[]).unwrap();
        } else {
            let host=match version {2=>frozen_hosts::FrozenHost::two(),3=>frozen_hosts::FrozenHost::three(),4=>frozen_hosts::FrozenHost::four(),_=>unreachable!()};
            host.open(&path,"migration-profile");
        }
        seed_historical_lineage(&path);
        inspect(&path,version);
        let before=Sqlite::open(&path).unwrap();
        let ledger=before.prepare("SELECT * FROM schema_migrations ORDER BY version").unwrap().query_map([],|row|Ok((row.get::<_,u32>(0)?,row.get::<_,String>(1)?,row.get::<_,i64>(2)?,row.get::<_,String>(3)?,row.get::<_,u32>(4)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap();
        drop(before);
        let route=format!("from{version}");
        let observation=runtime.as_ref().map(|_|SqliteStore::observe_schema5_open(&path,&route,"bj_11_storage","migration-profile","current"));
        if let Some(r)=&mut runtime {r.snapshot(&format!("{route}-before"),&path);r.event(&route,"writer_open_started",json!({"repository":"SqliteStore::open","now_ms":200,"server_id":"current"}));}
        let store=SqliteStore::open(&path,"migration-profile","current",200).unwrap();
        if let Some(guard)=&observation {guard.control.emit("caller_received_success","caller",json!({}));}
        store.checkpoint().unwrap();drop(store);
        inspect(&path,5);
        let db=Sqlite::open(&path).unwrap();
        for row in &ledger { assert_eq!(db.query_row("SELECT migration_sha256,applied_at_ms,app_build,previous_version FROM schema_migrations WHERE version=?1",[row.0],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,u32>(3)?))).unwrap(),(row.1.clone(),row.2,row.3.clone(),row.4)); }
        assert_eq!(db.query_row("SELECT max(version) FROM schema_migrations",[],|r|r.get::<_,u32>(0)).unwrap(),5);
        assert_eq!(db.query_row("SELECT source_text,processing_snapshot_json,selection_revision FROM reading_unit_revisions WHERE unit_id='legacy-unit'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?))).unwrap(),("旧来の原文。".into(),"{\"preserved\":true}".into(),1));
        assert_eq!(db.query_row("SELECT origin_kind,operation_id,persistence_kind,match_snapshot_json,selected_at_ms,member_count,state FROM source_selection_sets WHERE session_id='legacy-session'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,u32>(5)?,r.get::<_,String>(6)?))).unwrap(),("legacy_singleton".into(),None,"saved_snapshot".into(),"{\"schemaVersion\":999,\"unknown\":\"原字节\"}".into(),2,1,"sealed".into()));
        assert_eq!(db.query_row("SELECT source_id,selection_revision FROM current_source_selection_members WHERE session_id='legacy-session'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?))).unwrap(),("legacy-source".into(),1));
        drop(db);
        let backup=out.join(format!("from-{version}.sqlite3.pre-v5-200.sqlite3"));inspect(&backup,version);
        if let Some(r)=&mut runtime {r.snapshot(&format!("{route}-after"),&path);r.snapshot(&format!("{route}-backup"),&backup);r.route(&route,Some(&format!("{route}-before")),&format!("{route}-after"),5,Some(&format!("{route}-backup")),None);let guard=observation.as_ref().unwrap();guard.control.emit("independent_current5_db_ledger_invariants","independent_readback",json!({"before":r.snapshots[&format!("{route}-before")],"after":r.snapshots[&format!("{route}-after")],"backup":r.snapshots[&format!("{route}-backup")]}));parent_parts.insert(route.clone(),json!({"binding":guard.control.binding(),"timeline":guard.control.timeline()}));r.event(&route,"writer_open_committed",json!({"schema_version":5}));}
        drop(observation);
        let reopened_observation=if version==4&&runtime.is_some(){Some(SqliteStore::observe_schema5_open(&path,"reopen5","bj_11_storage","migration-profile","reopened"))}else{None};
        if version==4 {if let Some(r)=&mut runtime{r.snapshot("reopen5-before",&path);r.event("reopen5","writer_open_started",json!({"repository":"SqliteStore::open","now_ms":201,"server_id":"reopened"}));}}
        drop(SqliteStore::open(&path,"migration-profile","reopened",201).unwrap());
        if let Some(guard)=&reopened_observation {guard.control.emit("caller_received_success","caller",json!({}));inspect(&path,5);let r=runtime.as_mut().unwrap();r.snapshot("reopen5-after",&path);r.route("reopen5",Some("reopen5-before"),"reopen5-after",5,None,None);guard.control.emit("independent_current5_db_ledger_invariants","independent_readback",json!({"before":r.snapshots["reopen5-before"],"after":r.snapshots["reopen5-after"],"backup":null}));parent_parts.insert("reopen5".into(),json!({"binding":guard.control.binding(),"timeline":guard.control.timeline()}));r.event("reopen5","writer_open_committed",json!({"schema_version":5}));}
    }
    let old=frozen_hosts::FrozenHost::four();
    let before=std::fs::read(&fresh).unwrap();
    let result=if let Some(r)=&mut runtime {observe_frozen_old_four(r,&fresh,&old.binary,&mut parent_parts)}else{Process::new(old.binary).arg(&fresh).arg("migration-profile").arg("reject").output().unwrap()};
    assert!(result.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&result.stdout).unwrap()["error_code"],"unsupported_schema");
    assert_eq!(std::fs::read(&fresh).unwrap(),before);
    let invalid=out.join("unrepresentable-lineage.sqlite3");
    frozen_hosts::FrozenHost::four().open(&invalid,"migration-profile");
    seed_historical_lineage(&invalid);
    let db=Sqlite::open(&invalid).unwrap();
    db.execute("INSERT INTO source_selections VALUES('legacy-session',4294967296,'legacy-source','temporary','{}',3)",[]).unwrap();
    drop(db);
    inspect(&invalid,4);
    let bytes=std::fs::read(&invalid).unwrap();
    let error=match SqliteStore::open(&invalid,"migration-profile","rejected",200) {Ok(_)=>panic!("unrepresentable old lineage must be rejected before upgrade"),Err(error)=>error};
    assert_eq!(error.code,"selection_lineage_corrupt");
    assert_eq!(std::fs::read(&invalid).unwrap(),bytes);
    assert!(!out.join("unrepresentable-lineage.sqlite3.pre-v5-200.sqlite3").exists());
    if let Some(mut r)=runtime {
        execute_kill_and_restore(&mut r,&out,&mut parent_parts);
        let child=execute_private_fault_children(&mut r);write_new(&r.base.join("child-proof-parts.json"),&child);write_new(&r.base.join("parent-timeline-parts.json"),&Value::Object(parent_parts));r.save();
    }
}

#[test]
fn bj_12_storage() {
    let f=Fixture::new("bj12",true);
    let stale=f.input("stale-head",&["source-a"],false);
    let first=f.commit("new-head",&["source-a","source-b"],false,20);
    assert_eq!(f.store.commit_source_selection(stale,21).unwrap_err().code,"selection_head_conflict");
    let before=f.scalar("SELECT count(*) FROM source_selections");
    for (operation,ids) in [("empty",vec![]),("duplicate",vec!["source-a","source-a"]),("missing",vec!["absent"])] {
        let input=f.input(operation,&ids,false);
        assert!(f.store.commit_source_selection(input,22).is_err());
        assert_eq!(f.scalar("SELECT count(*) FROM source_selections"),before);
    }
    let mut wrong=f.input("wrong-target",&["source-a"],false);
    wrong.active_session.process_created_u64="2".into();
    assert_eq!(f.store.commit_source_selection(wrong,23).unwrap_err().code,"stale_capture_target");
    assert_eq!(f.store.validate_reading_window_effect(first.effect_authority.clone()).unwrap().selection,first.selection);
    f.commit("different-set",&["source-b"],false,24);
    assert_eq!(f.store.validate_reading_window_effect(first.effect_authority).unwrap_err().code,"stale_reading_window_effect");
    let mut rollback=Fixture::new("bj12-rollback",false);
    rollback.commit("prior",&["source-a"],false,20);
    let open=rollback.text("source-a","未完成の原文",1000).visible_version.unwrap();
    let before=(rollback.scalar("SELECT count(*) FROM source_selection_sets"),rollback.scalar("SELECT count(*) FROM source_selections"));
    let db=Sqlite::open(&rollback.path).unwrap();
    db.execute_batch("CREATE TRIGGER controlled_selection_failure BEFORE INSERT ON source_selection_members WHEN NEW.member_ordinal=1 BEGIN SELECT RAISE(ABORT,'controlled_actual_second_member_failure'); END;").unwrap();
    let input=rollback.input("retry-after-rollback",&["source-a","source-b"],false);
    assert!(rollback.store.commit_source_selection(input.clone(),1001).unwrap_err().message.contains("controlled_actual_second_member_failure"));
    assert_eq!((rollback.scalar("SELECT count(*) FROM source_selection_sets"),rollback.scalar("SELECT count(*) FROM source_selections")),before);
    assert_eq!(db.query_row("SELECT state FROM reading_units WHERE unit_id=?1",[&open.unit_id],|r|r.get::<_,String>(0)).unwrap(),"open","sealing the old unit must roll back with the failed member");
    assert_eq!(rollback.scalar("SELECT count(*) FROM operation_receipts WHERE operation_id='retry-after-rollback'"),0);
    db.execute_batch("DROP TRIGGER controlled_selection_failure").unwrap();
    assert_eq!(rollback.store.commit_source_selection(input,1002).unwrap().selection.members.len(),2);
}

#[test]
fn bj_17_storage() {
    for work in [false,true] {
    let mut f=Fixture::new(if work {"bj17-work"} else {"bj17-legacy"},false);
    configured(&f.store);
    let original=f.commit("set-a-b",&["source-a","source-b"],false,20);
    let a=f.text("source-a","甲。",1000).visible_version.unwrap();
    let b=f.text("source-b","乙。",3000).visible_version.unwrap();
    if work {
        assert_eq!(f.store.prepare_work_capture_tasks(a.clone(),"auto-work-a".into(),4002).unwrap().len(),1);
        assert_eq!(f.store.prepare_work_capture_tasks(b.clone(),"auto-work-b".into(),4003).unwrap().len(),1);
    } else {
        assert_eq!(f.store.prepare_capture_tasks(a.clone(),"auto-old-a".into(),4000).unwrap().len(),1);
        assert_eq!(f.store.prepare_capture_tasks(b.clone(),"auto-old-b".into(),4001).unwrap().len(),1);
    }
    f.commit("set-b",&["source-b"],false,5000);
    assert!(f.store.prepare_capture_tasks(a.clone(),"old-disallowed".into(),5001).unwrap().is_empty());
    let returned=f.commit("set-a-b-again",&["source-a","source-b"],false,5002);
    assert!(returned.selection.members.iter().all(|m|original.selection.members.iter().all(|old|old.selection_revision!=m.selection_revision)));
    for version in [a,b] {
        assert!(f.store.prepare_capture_tasks(version.clone(),format!("stale-{}",version.unit_id),6000).unwrap().is_empty());
        assert!(f.store.prepare_work_capture_tasks(version.clone(),format!("stale-work-{}",version.unit_id),6001).unwrap().is_empty());
    }
    let new=f.text("source-a","新しい甲。",7000).visible_version.unwrap();
    if work { assert_eq!(f.store.prepare_work_capture_tasks(new,"new-authorized".into(),8000).unwrap().len(),1); }
    else { assert_eq!(f.store.prepare_capture_tasks(new,"new-authorized".into(),8000).unwrap().len(),1); }
    let legacy=command(&f.store,"legacy-single",Command::SelectSource {source_id:"source-b".into()},9000);
    assert!(!legacy.replayed);
    assert_eq!(f.projection().current.unwrap().members.len(),1);
    }
    finalize_runtime_artifact();
}

fn wall_ms() -> u64 {
    u64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()).unwrap()
}
fn artifact(path:&Path,base:&Path)->Value {
    let path=path.canonicalize().unwrap();
    json!({"path":path.strip_prefix(base).unwrap().to_string_lossy(),"sha256":content_hash(std::fs::read(&path).unwrap())})
}
fn table_rows(path:&Path,table:&str)->Vec<Value> {
    let db=Sqlite::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let sql=match table {
        "source_selection_sets"=>"SELECT * FROM source_selection_sets WHERE origin_kind='explicit_set' AND state='sealed' ORDER BY session_id,selection_set_revision",
        "source_selection_members"=>"SELECT * FROM source_selection_members ORDER BY session_id,selection_set_revision,member_ordinal",
        _=>panic!("unregistered evidence table"),
    };
    let mut statement=db.prepare(sql).unwrap();
    let columns=statement.column_names().into_iter().map(str::to_owned).collect::<Vec<_>>();
    statement.query_map([],|row|{
        let values=columns.iter().enumerate().map(|(index,name)|{
            let value=match row.get_ref(index).unwrap(){rusqlite::types::ValueRef::Null=>Value::Null,rusqlite::types::ValueRef::Integer(value)=>json!(value),rusqlite::types::ValueRef::Text(value)=>json!(std::str::from_utf8(value).unwrap()),_=>panic!("unexpected selection field carrier")};
            (name.clone(),value)
        }).collect::<serde_json::Map<_,_>>();Ok(Value::Object(values))
    }).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
}
fn write_selection_artifact(path:&Path) {
    let Some(base)=std::env::var_os("LUNA_BINDING_OUTPUT").map(PathBuf::from)else{return;};
    let base=base.canonicalize().unwrap();
    let binary=std::env::current_exe().unwrap();
    let value=json!({"nonce":std::env::var("LUNA_BINDING_NONCE").unwrap(),"source_tree_sha256":std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap(),"artifact_sha256":content_hash(std::fs::read(binary).unwrap()),"snapshot":artifact(path,&base),"rows":{"sets":table_rows(path,"source_selection_sets"),"members":table_rows(path,"source_selection_members")}});
    write_new(&base.join("selection-artifact.json"),&value);
}
fn write_new(path:&Path,value:&Value) {
    use std::io::Write;
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    file.write_all(&serde_json::to_vec_pretty(value).unwrap()).unwrap();file.sync_all().unwrap();
}
struct RuntimeCapture {
    base:PathBuf, nonce:String, source:String, artifact_sha:String,
    started:u64, events:Vec<Value>, snapshots:serde_json::Map<String,Value>, routes:Vec<Value>, sqlite:Value,
}
impl RuntimeCapture {
    fn requested()->Option<Self> {
        let base=PathBuf::from(std::env::var_os("LUNA_BINDING_OUTPUT")?);std::fs::create_dir_all(&base).unwrap();
        let base=base.canonicalize().unwrap();assert!(base.starts_with(root().join(".build")));
        let nonce=std::env::var("LUNA_BINDING_NONCE").unwrap();let source=std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap();
        assert_eq!(nonce.len(),32);assert_eq!(source.len(),64);
        let binary=std::env::current_exe().unwrap();
        Some(Self{base,nonce,source,artifact_sha:content_hash(std::fs::read(binary).unwrap()),started:wall_ms(),events:vec![],snapshots:serde_json::Map::new(),routes:vec![],sqlite:Value::Null})
    }
    fn event_at(&mut self,route:&str,event:&str,at:u64,facts:Value){
        let value=json!({"sequence":self.events.len()+1,"nonce":self.nonce,"source_tree_sha256":self.source,"artifact_sha256":self.artifact_sha,"route":route,"event":event,"at_ms":at,"facts":facts});
        println!("LUNA_STORAGE_V5_EVENT {}",serde_json::to_string(&value).unwrap());self.events.push(value);
    }
    fn event(&mut self,route:&str,event:&str,facts:Value){self.event_at(route,event,wall_ms(),facts)}
    fn snapshot(&mut self,id:&str,path:&Path){
        let dir=self.base.join("snapshots");std::fs::create_dir_all(&dir).unwrap();let dest=dir.join(format!("{id}.sqlite3"));assert!(!dest.exists());
        let db=Sqlite::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();db.backup("main",&dest,None).unwrap();drop(db);
        let closed=Sqlite::open(&dest).unwrap();closed.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE").unwrap();assert_eq!(closed.query_row("PRAGMA integrity_check",[],|r|r.get::<_,String>(0)).unwrap(),"ok");drop(closed);
        assert!(self.snapshots.insert(id.into(),artifact(&dest,&self.base)).is_none());
    }
    fn route(&mut self,id:&str,before:Option<&str>,after:&str,version:u32,backup:Option<&str>,error:Option<&str>){
        self.routes.push(json!({"id":id,"before":before,"after":after,"after_version":version,"backup":backup,"error_code":error}));
    }
    fn save(self){
        use std::io::Write;
        let trace=self.base.join("migration-route-events.jsonl");let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&trace).unwrap();
        for event in &self.events{writeln!(file,"{}",serde_json::to_string(event).unwrap()).unwrap();}file.sync_all().unwrap();
        write_new(&self.base.join("migration-artifacts.json"),&json!({"nonce":self.nonce,"source_tree_sha256":self.source,"artifact_sha256":self.artifact_sha,"started_at_ms":self.started,"snapshots":self.snapshots,"routes":self.routes,"route_trace":artifact(&trace,&self.base),"sqlite":self.sqlite}));
    }
}

fn root_artifact(path:&Path)->Value {artifact(path,&root())}
fn utc_timestamp(ms:u64)->String {
    let seconds=libc::time_t::try_from(ms/1000).unwrap();let mut value=std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: seconds is initialized and value has space/alignment for tm; successful gmtime_r initializes value.
    assert!(!unsafe{libc::gmtime_r(&seconds,value.as_mut_ptr())}.is_null());
    // SAFETY: gmtime_r succeeded immediately above and initialized these fields.
    let value=unsafe{value.assume_init()};format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",value.tm_year+1900,value.tm_mon+1,value.tm_mday,value.tm_hour,value.tm_min,value.tm_sec,ms%1000)
}
fn fixture_environment()->Value {json!({"LUNA_BINDING_NONCE":std::env::var("LUNA_BINDING_NONCE").unwrap(),"LUNA_BINDING_OUTPUT":std::env::var("LUNA_BINDING_OUTPUT").unwrap(),"LUNA_BINDING_SOURCE_TREE":std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap()})}
fn run_owned_child(base:&Path,label:&str,command:&[String],launch:bool)->(std::process::Output,Value) {
    let start=wall_ms();let child=Process::new(&command[0]).args(&command[1..]).current_dir(root()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();let pid=child.id();let output=child.wait_with_output().unwrap();let end=wall_ms();let stdout=base.join(format!("{label}.stdout"));let stderr=base.join(format!("{label}.stderr"));std::fs::write(&stdout,&output.stdout).unwrap();std::fs::write(&stderr,&output.stderr).unwrap();let record=json!({"command":command,"cwd":".","pid":pid,"timed_out":false,"started_at":utc_timestamp(start),"finished_at":utc_timestamp(end),"exit_code":output.status.code().unwrap(),"fixture_environment":fixture_environment(),"stdout":root_artifact(&stdout),"stderr":root_artifact(&stderr)});let execution=base.join(format!("{label}.execution.json"));write_new(&execution,&record);let reference=root_artifact(&execution);if launch{println!("LUNA_STORAGE_V5_CHILD_LAUNCH {}",json!({"pid":pid,"command":command,"execution":reference}));}assert!(output.status.success(),"actual child {label} failed: {}",String::from_utf8_lossy(&output.stderr));(output,reference)
}
fn raw_frames(raw:&[u8],marker:&str,exact:&str)->Vec<Value> {let prefix=format!("test {exact} ... {marker}");String::from_utf8_lossy(raw).lines().filter_map(|line|line.strip_prefix(marker).or_else(||line.strip_prefix(&prefix))).map(|line|serde_json::from_str(line).unwrap()).collect()}
fn execute_private_fault_children(runtime:&mut RuntimeCapture)->Value {
    let base=runtime.base.clone();let source_path=base.join("child-source-files.json");let script="import json,sys;from pathlib import Path;sys.path.insert(0,str(Path(sys.argv[1])/'scripts/quality'));from gate_core import source_files;from binding_phase_guard import tree_sha;s=source_files(Path(sys.argv[1]));Path(sys.argv[2]).write_text(json.dumps(s,sort_keys=True,indent=2)+'\\n');print(tree_sha(s))";
    let source=Process::new("python3").args(["-c",script]).arg(root()).arg(&source_path).output().unwrap();assert!(source.status.success());assert_eq!(String::from_utf8(source.stdout).unwrap().trim(),runtime.source,"source changed before actual child compilation; no same-source proof");
    let command=["cargo","test","--locked","--offline","-p","app-infra","--lib","--no-run","--message-format=json"].map(str::to_owned);let (compiled,compilation)=run_owned_child(&base,"child-compile",&command,true);
    let mut executables=String::from_utf8_lossy(&compiled.stdout).lines().filter_map(|line|serde_json::from_str::<Value>(line).ok()).filter(|m|m["reason"]=="compiler-artifact"&&m["target"]["name"]=="app_infra"&&m["target"]["kind"]==json!(["lib"])&&m["profile"]["test"]==true&&m["executable"].is_string()).map(|m|PathBuf::from(m["executable"].as_str().unwrap())).collect::<Vec<_>>();assert_eq!(executables.len(),1);let executable=base.join("actual-schema5-child-libtest");std::fs::copy(executables.remove(0),&executable).unwrap();let binary=root_artifact(&executable);
    let (_,listing)=run_owned_child(&base,"child-list",&[executable.to_string_lossy().into_owned(),"--list".into(),"--format".into(),"terse".into()],true);
    let tests=["storage::database::source_selection_migration_fault::actual_schema5_host_rolls_back_after_ddl_and_recovers_same_original_profile","storage::database::source_selection_migration_fault::actual_schema5_host_readonly_and_busy_leave_the_original_unchanged","storage::database::source_selection_migration_fault::actual_schema5_host_full_isolated_volume_preserves_the_old_database"];
    let mut executions=vec![];let mut fault_records=serde_json::Map::new();
    for (index,name) in tests.iter().enumerate(){let (result,reference)=run_owned_child(&base,&format!("child-exact-{index}"),&[executable.to_string_lossy().into_owned(),"--exact".into(),(*name).into(),"--nocapture".into()],true);executions.push(reference);
        for record in raw_frames(&result.stdout,"LUNA_STORAGE_V5_FAULT ",name){let route=record["route"].as_str().unwrap();let record_path=root().join(record["timeline"]["path"].as_str().unwrap()).parent().unwrap().join(format!("{route}.json"));assert_eq!(serde_json::from_slice::<Value>(&std::fs::read(&record_path).unwrap()).unwrap(),record);assert!(fault_records.insert(route.into(),root_artifact(&record_path)).is_none());
            let timeline:Value=serde_json::from_slice(&std::fs::read(root().join(record["timeline"]["path"].as_str().unwrap())).unwrap()).unwrap();let code=if route=="busy"{"storage_busy"}else{"storage_failure"};let fault=match route{"rollback"=>"transaction_check","readonly"=>"readonly","busy"=>"exclusive_lock","enospc"=>"filesystem_full",_=>panic!("unregistered child route")};
            for event in timeline["events"].as_array().unwrap(){let (kind,facts)=match event["event_kind"].as_str().unwrap(){"api_entry"=>("writer_open_started",json!({"repository":"SqliteStore::open","now_ms":100,"server_id":"controlled-fault"})),"fault_applied"=>("fault_armed",json!({"fault":fault,"holder_pid":record["child_pid"],"lock":"BEGIN IMMEDIATE","read_only":route=="readonly","available_bytes":record["os_facts"]["available_bytes_at_fault"],"volume":record["os_facts"]["device"]})),"native_failure"=>("writer_open_failed",json!({"error_code":code,"sqlite_code":event["payload"]["native_primary_code"]})),"transaction_started" if route=="rollback"=>("migration_transaction_started",json!({})),"ddl_applied" if route=="rollback"=>("migration_ddl_applied",json!({})),"transaction_scope_exited_after_drop" if route=="rollback"=>("transaction_rolled_back",json!({})),_=>continue};runtime.event_at(route,kind,event["observed_wall_ms"].as_u64().unwrap(),facts);}
            for key in ["before","after","backup"]{let id=format!("{route}-{key}");assert!(runtime.snapshots.insert(id,artifact(&root().join(record[key]["path"].as_str().unwrap()),&base)).is_none());}runtime.route(route,Some(&format!("{route}-before")),&format!("{route}-after"),4,Some(&format!("{route}-backup")),Some(code));
        }
    }
    assert_eq!(fault_records.len(),4);let manifest=PathBuf::from(std::env::var_os("FP_STORAGE_OLD_STORE4_READER_MANIFEST").unwrap());json!({"kind":"schema5-child-execution-parts-not-final/v1","source_files":root_artifact(&source_path),"cargo_lock":root_artifact(&root().join("Cargo.lock")),"old_writer_manifest":root_artifact(&manifest),"compilation":compilation,"binary":binary,"listing":listing,"executions":executions,"fault_records":fault_records,"parent_pid":std::process::id(),"parent_binary":root_artifact(&std::env::current_exe().unwrap())})
}
fn file_identity(path:&Path)->Value {use std::os::unix::fs::MetadataExt;let metadata=std::fs::metadata(path).unwrap();json!({"device":metadata.dev(),"inode":metadata.ino()})}
fn observe_frozen_old_four(runtime:&mut RuntimeCapture,path:&Path,binary:&Path,parts:&mut serde_json::Map<String,Value>)->std::process::Output {
    let original=std::fs::read(path).unwrap();let before_identity=file_identity(path);let source=runtime.source.clone();let nonce=runtime.nonce.clone();let invocation=uuid::Uuid::new_v4().to_string();let observer=std::process::id();let clock=std::time::Instant::now();
    let snapdir=runtime.base.join("snapshots");std::fs::create_dir_all(&snapdir).unwrap();let before=snapdir.join("old4-before.sqlite3");std::fs::copy(path,&before).unwrap();runtime.snapshots.insert("old4-before".into(),artifact(&before,&runtime.base));runtime.event("old4-rejects5","writer_open_started",json!({"repository":"SqliteStore::open","now_ms":100,"server_id":"frozen-old4"}));
    let command=vec![binary.to_string_lossy().into_owned(),path.to_string_lossy().into_owned(),"migration-profile".into(),"reject".into()];let start=wall_ms();let child=Process::new(binary).arg(path).args(["migration-profile","reject"]).current_dir(root()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();let reader=child.id();let spawn_clock=(wall_ms(),clock.elapsed().as_nanos());let result=child.wait_with_output().unwrap();let end=wall_ms();let wait_clock=(wall_ms(),clock.elapsed().as_nanos());assert!(result.status.success());let actual:Value=serde_json::from_slice(&result.stdout).unwrap();assert_eq!(actual["opened"],false);assert_eq!(actual["schema_version"],4);assert_eq!(actual["error_code"],"unsupported_schema");
    let stdout=runtime.base.join("old4-reject.stdout");let stderr=runtime.base.join("old4-reject.stderr");std::fs::write(&stdout,&result.stdout).unwrap();std::fs::write(&stderr,&result.stderr).unwrap();let stdout=root_artifact(&stdout);let stderr=root_artifact(&stderr);let capture_clock=(wall_ms(),clock.elapsed().as_nanos());let execution_path=runtime.base.join("old4-reject.execution.json");write_new(&execution_path,&json!({"command":command,"cwd":".","pid":reader,"parent_pid":observer,"timed_out":false,"started_at":utc_timestamp(start),"finished_at":utc_timestamp(end),"exit_code":result.status.code().unwrap(),"fixture_environment":fixture_environment(),"stdout":stdout,"stderr":stderr}));let execution=root_artifact(&execution_path);
    assert_eq!(std::fs::read(path).unwrap(),original);let after_identity=file_identity(path);assert_eq!(before_identity,after_identity);let after=snapdir.join("old4-after.sqlite3");std::fs::copy(path,&after).unwrap();runtime.snapshots.insert("old4-after".into(),artifact(&after,&runtime.base));
    // Independently inspect the complete current schema and every table value;
    // the frozen binary's error alone does not prove unchanged storage.
    let verify="import sys;from pathlib import Path;sys.path.insert(0,sys.argv[1]+'/scripts/quality');import storage_schema_v5 as s;a=s._snapshot(Path(sys.argv[1]),Path(sys.argv[1]),{'path':sys.argv[2],'sha256':s.digest(Path(sys.argv[2]).read_bytes())},5);b=s._snapshot(Path(sys.argv[1]),Path(sys.argv[1]),{'path':sys.argv[3],'sha256':s.digest(Path(sys.argv[3]).read_bytes())},5);s._same_database(a,b);a.close();b.close()";
    // Pass owned relative paths so the independent inspector's path checks stay active.
    let verified=Process::new("python3").args(["-c",verify]).arg(root()).arg(before.strip_prefix(root()).unwrap()).arg(after.strip_prefix(root()).unwrap()).current_dir(root()).output().unwrap();assert!(verified.status.success(),"{}",String::from_utf8_lossy(&verified.stderr));let verified_clock=(wall_ms(),clock.elapsed().as_nanos());
    let binding=json!({"route":"old4-rejects5","case_id":"BJ-11","invocation_id":invocation,"exact_test":"bj_11_storage","nonce":nonce,"source_tree_sha256":source,"observer_pid":observer,"reader_pid":reader,"profile_id":"migration-profile","original_database_path":path,"database_file_device_identity":before_identity});let mut observations=vec![];
    let rows=[("reader_spawned",spawn_clock,json!({"command":command,"binary":root_artifact(binary),"execution":execution})),("reader_waited",wait_clock,json!({"exit_code":0})),("reader_result_captured",capture_clock,json!({"stdout":stdout,"stderr":stderr,"error_code":"unsupported_schema"})),("original_database_verified",verified_clock,json!({"before":runtime.snapshots["old4-before"],"after":runtime.snapshots["old4-after"],"original_database_path":path,"before_identity":before_identity,"after_identity":after_identity}))];
    for (kind,(wall,mono),payload) in rows {let event=json!({"kind":kind,"sequence":observations.len()+1,"monotonic_ns":u64::try_from(mono).unwrap(),"observed_wall_ms":wall,"observer_pid":observer,"reader_pid":reader,"route":"old4-rejects5","invocation_id":invocation,"nonce":nonce,"source_tree_sha256":source,"payload":payload});println!("LUNA_STORAGE_V5_OLD4_OBSERVATION {}",event);observations.push(event);}
    parts.insert("old4-rejects5".into(),json!({"kind":"schema5-frozen-old-reader-observation-parts/v1","binding":binding,"old_writer_manifest":root_artifact(&PathBuf::from(std::env::var_os("FP_STORAGE_OLD_STORE4_READER_MANIFEST").unwrap())),"reader_execution":execution,"observations":observations}));runtime.route("old4-rejects5",Some("old4-before"),"old4-after",5,None,Some("unsupported_schema"));runtime.event("old4-rejects5","old_writer_rejected",json!({"error_code":"unsupported_schema","pid":reader}));result
}
fn monotonic_ns()->u64 {let mut value=std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: value has valid space/alignment; successful clock_gettime initializes it.
    assert_eq!(unsafe{libc::clock_gettime(libc::CLOCK_MONOTONIC,value.as_mut_ptr())},0);
    // SAFETY: clock_gettime succeeded immediately above and initialized value.
    let value=unsafe{value.assume_init()};u64::try_from(value.tv_sec).unwrap()*1_000_000_000+u64::try_from(value.tv_nsec).unwrap()}
fn observer_clock()->(u64,u64) {(wall_ms(),monotonic_ns())}
fn external_boundary(binding:&Value,sequence:usize,kind:&str,clock:(u64,u64),payload:Value)->Value {let mut event=binding.clone();event["event_kind"]=json!(kind);event["event_sequence"]=json!(sequence);event["monotonic_ns"]=json!(clock.1);event["observed_wall_ms"]=json!(clock.0);event["actual_thread_id"]=json!(format!("{}:{:?}",std::process::id(),std::thread::current().id()));event["actual_phase"]=json!("parent_external_observation");event["payload"]=payload;event}
fn kill_child_test_host(path:&Path) {
    // Only this fixed test-driver branch reads this control. The product library
    // has neither an environment selector nor a public/debug fault API.
    let mut release=String::new();std::io::stdin().read_line(&mut release).unwrap();assert_eq!(release,"start actual migration\n");
    let _observation=SqliteStore::observe_schema5_kill_boundary(path,"migration-profile","killed");
    let _store=SqliteStore::open(path,"migration-profile","killed",250).unwrap();panic!("the parent must SIGKILL at the actual DDL boundary before returning");
}
fn assert_same_database_files(before:&Path,after:&Path,version:u32) {
    let script="import sys;from pathlib import Path;sys.path.insert(0,sys.argv[1]+'/scripts/quality');import storage_schema_v5 as s;r=Path(sys.argv[1]);refs=[{'path':str(Path(p).relative_to(r)),'sha256':s.digest(Path(p).read_bytes())} for p in sys.argv[2:4]];a,b=[s._snapshot(r,r,p,int(sys.argv[4])) for p in refs];s._same_database(a,b);a.close();b.close()";
    let result=Process::new("python3").args(["-c",script]).arg(root()).arg(before).arg(after).arg(version.to_string()).output().unwrap();assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
}
fn execute_kill_and_restore(runtime:&mut RuntimeCapture,out:&Path,parts:&mut serde_json::Map<String,Value>) {
    use std::io::{BufRead,Read,Write};use std::os::unix::process::ExitStatusExt;
    let path=out.join("actual-kill-old-four.sqlite3");frozen_hosts::FrozenHost::four().open(&path,"migration-profile");seed_historical_lineage(&path);inspect(&path,4);runtime.snapshot("kill-before",&path);runtime.event("kill","writer_open_started",json!({"repository":"SqliteStore::open","now_ms":250,"server_id":"killed"}));
    let binary=std::env::current_exe().unwrap();let command=vec![binary.to_string_lossy().into_owned(),"--exact".into(),"bj_11_storage".into(),"--nocapture".into()];let start=wall_ms();let mut child=Process::new(&binary).args(&command[1..]).env("LUNA_STORAGE_SCHEMA5_KILL_DATABASE",&path).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).current_dir(root()).spawn().unwrap();let pid=child.id();let started=observer_clock();runtime.event("kill","child_started",json!({"pid":pid,"command":command}));child.stdin.take().unwrap().write_all(b"start actual migration\n").unwrap();let mut reader=std::io::BufReader::new(child.stdout.take().unwrap());let mut raw=Vec::new();let mut observed=vec![];
    loop {let mut line=String::new();assert!(reader.read_line(&mut line).unwrap()>0,"actual child exited before DDL boundary");raw.extend_from_slice(line.as_bytes());let events=raw_frames(line.as_bytes(),"LUNA_STORAGE_V5_BOUNDARY ","bj_11_storage");let done=events.iter().any(|e|e["event_kind"]=="ddl_applied");observed.extend(events);if done{break;}}
    let mut binding=observed[0].clone();for key in ["event_kind","event_sequence","monotonic_ns","observed_wall_ms","actual_phase","payload","actual_thread_id"]{binding.as_object_mut().unwrap().remove(key);}assert_eq!(binding["child_pid"],pid);assert_eq!(binding["child_ppid"],std::process::id());
    runtime.event("kill","child_boundary_observed",json!({"pid":pid,"boundary":"migration_transaction"}));let kill=observer_clock();child.kill().unwrap();runtime.event_at("kill","kill_sent",kill.0,json!({"pid":pid,"signal":9}));let result=child.wait_with_output().unwrap();let waited=observer_clock();assert_eq!(result.status.signal(),Some(9));runtime.event_at("kill","child_waited",waited.0,json!({"pid":pid,"exit_signal":result.status.signal()}));reader.read_to_end(&mut raw).unwrap();let end=wall_ms();let stdout=runtime.base.join("actual-kill-child.stdout");let stderr=runtime.base.join("actual-kill-child.stderr");std::fs::write(&stdout,&raw).unwrap();std::fs::write(&stderr,&result.stderr).unwrap();let execution=runtime.base.join("actual-kill-child.execution.json");write_new(&execution,&json!({"command":command,"cwd":".","pid":pid,"parent_pid":std::process::id(),"timed_out":false,"started_at":utc_timestamp(start),"finished_at":utc_timestamp(end),"exit_code":result.status.code(),"exit_signal":result.status.signal(),"fixture_environment":fixture_environment(),"test_driver_control":{"LUNA_STORAGE_SCHEMA5_KILL_DATABASE":path},"stdout":root_artifact(&stdout),"stderr":root_artifact(&stderr)}));
    let mut events=vec![external_boundary(&binding,1,"actual_writer_started",started,json!({"pid":pid,"command":command,"binary":root_artifact(&binary),"execution":root_artifact(&execution)}))];events.extend(observed);events.push(external_boundary(&binding,events.len()+1,"actual_kill_command",kill,json!({"pid":pid,"signal":9,"method":"std::process::Child::kill"})));events.push(external_boundary(&binding,events.len()+1,"actual_wait_status",waited,json!({"pid":pid,"exit_signal":9,"execution":root_artifact(&execution)})));
    inspect(&path,4);runtime.snapshot("kill-after",&path);assert_same_database_files(&runtime.base.join(runtime.snapshots["kill-before"]["path"].as_str().unwrap()),&runtime.base.join(runtime.snapshots["kill-after"]["path"].as_str().unwrap()),4);let backup=path.with_file_name("actual-kill-old-four.sqlite3.pre-v5-250.sqlite3");inspect(&backup,4);assert_same_database_files(&runtime.base.join(runtime.snapshots["kill-before"]["path"].as_str().unwrap()),&backup,4);runtime.route("kill",Some("kill-before"),"kill-after",4,None,Some("process_killed"));events.push(external_boundary(&binding,events.len()+1,"independent_post_kill_db_ledger_backup_invariants",observer_clock(),json!({"before":runtime.snapshots["kill-before"],"after":runtime.snapshots["kill-after"],"backup":null,"actual_preupgrade_backup":root_artifact(&backup)})));
    let restored=out.join("restored-from-real-backup.sqlite3");std::fs::copy(&backup,&restored).unwrap();inspect(&restored,4);assert_same_database_files(&backup,&restored,4);runtime.snapshot("restore-before",&restored);runtime.snapshot("restore-backup",&backup);runtime.event("restore","writer_open_started",json!({"repository":"SqliteStore::open","now_ms":300,"server_id":"restored"}));runtime.event("restore","backup_restored",json!({"backup_snapshot":"restore-backup","source":root_artifact(&backup),"destination":restored}));let guard=SqliteStore::observe_schema5_open(&restored,"restore","bj_11_storage","migration-profile","restored");drop(SqliteStore::open(&restored,"migration-profile","restored",300).unwrap());guard.control.emit("caller_received_success","caller",json!({}));inspect(&restored,5);runtime.snapshot("restore-after",&restored);let current_backup=out.join("restored-from-real-backup.sqlite3.pre-v5-300.sqlite3");inspect(&current_backup,4);assert_same_database_files(&backup,&current_backup,4);runtime.route("restore",Some("restore-before"),"restore-after",5,Some("restore-backup"),None);guard.control.emit("independent_current5_db_ledger_invariants","independent_readback",json!({"before":runtime.snapshots["restore-before"],"after":runtime.snapshots["restore-after"],"backup":runtime.snapshots["restore-backup"],"current_writer_preupgrade_backup":root_artifact(&current_backup)}));parts.insert("restore".into(),json!({"binding":guard.control.binding(),"timeline":guard.control.timeline()}));runtime.event("restore","writer_open_committed",json!({"schema_version":5}));drop(guard);
    events.push(external_boundary(&binding,events.len()+1,"actual_recovery",observer_clock(),json!({"snapshot":runtime.snapshots["restore-after"],"actual_restore_database_path":restored})));for event in &events{println!("LUNA_STORAGE_V5_BOUNDARY {}",event);}parts.insert("kill".into(),json!({"binding":binding,"timeline":{"kind":"private-schema5-production-timeline/v1","events":events,"not_reached":{}}}));
}
fn finalize_runtime_artifact() {
    let Some(base)=std::env::var_os("LUNA_BINDING_OUTPUT").map(PathBuf::from)else{return;};let base=base.canonicalize().unwrap();if !base.join("migration-artifacts.json").is_file(){return;}
    let nonce=std::env::var("LUNA_BINDING_NONCE").unwrap();let source=std::env::var("LUNA_BINDING_SOURCE_TREE").unwrap();let executable=std::env::current_exe().unwrap().canonicalize().unwrap();let artifact_sha=content_hash(std::fs::read(&executable).unwrap());
    let migrations:Value=serde_json::from_slice(&std::fs::read(base.join("migration-artifacts.json")).unwrap()).unwrap();let selection:Value=serde_json::from_slice(&std::fs::read(base.join("selection-artifact.json")).unwrap()).unwrap();for input in [&migrations,&selection]{assert_eq!(input["nonce"],nonce);assert_eq!(input["source_tree_sha256"],source);assert_eq!(input["artifact_sha256"],artifact_sha);}
    let parent_execution=base.parent().unwrap().join("storage-BJ-11.execution.json");let actual_parent:Value=serde_json::from_slice(&std::fs::read(&parent_execution).expect("actual outer bj11 execution receipt must precede final exact bj17; missing is blocked")).unwrap();let parent_ref=root_artifact(&parent_execution);let mut child:Value=serde_json::from_slice(&std::fs::read(base.join("child-proof-parts.json")).unwrap()).unwrap();assert_eq!(child["parent_pid"],actual_parent["pid"]);assert_eq!(child["parent_binary"],root_artifact(&executable));assert_eq!(actual_parent["command"],json!([executable,"--exact","bj_11_storage","--nocapture"]));let parent_pid=child.as_object_mut().unwrap().remove("parent_pid").unwrap();let parent_binary=child.as_object_mut().unwrap().remove("parent_binary").unwrap();child["kind"]=json!("schema5-child-execution/v1");child["parent"]=json!({"execution":parent_ref,"binary":parent_binary,"pid":parent_pid});let child_path=base.join("child-proof.json");write_new(&child_path,&child);
    let parts:Value=serde_json::from_slice(&std::fs::read(base.join("parent-timeline-parts.json")).unwrap()).unwrap();let mut timelines=serde_json::Map::new();for (route,part) in parts.as_object().unwrap(){let mut envelope=part.clone();envelope["execution"]=parent_ref.clone();envelope["kind"]=json!(if route=="old4-rejects5"{"schema5-frozen-old-reader-observation/v1"}else{"schema5-parent-route-timeline/v1"});let path=base.join(format!("parent-{route}-timeline.json"));write_new(&path,&envelope);timelines.insert(route.clone(),root_artifact(&path));}
    // Failed precreation invocations never get an invented positive identity.
    if !timelines.contains_key("fresh") {write_new(&base.join("storage-runtime-blocked.json"),&json!({"kind":"actual-storage-v2-producer-blocked/v1","nonce":nonce,"source_tree_sha256":source,"artifact_sha256":artifact_sha,"reason":"fresh original callback/first-identity/sealed provenance unavailable; incomplete invocation remains blocked","child_proof":root_artifact(&child_path),"production_timelines":timelines,"migration_artifacts":root_artifact(&base.join("migration-artifacts.json")),"product_acceptance_promoted":false}));return;}
    let fresh=base.join(migrations["snapshots"]["fresh-after"]["path"].as_str().unwrap());let script="import sys,json;from pathlib import Path;sys.path.insert(0,sys.argv[1]+'/scripts/quality');import storage_schema_v5 as s;c=s._readonly(Path(sys.argv[2]));facts=s._boundary_facts(c);c.close();print(json.dumps(facts,ensure_ascii=False))";let replay=Process::new("python3").args(["-c",script]).arg(root()).arg(&fresh).output().unwrap();assert!(replay.status.success(),"{}",String::from_utf8_lossy(&replay.stderr));let facts:Value=serde_json::from_slice(&replay.stdout).unwrap();let release=Process::new("uname").arg("-r").output().unwrap();assert!(release.status.success());let old_ref=|relative:&str|root_artifact(&root().join(relative));let mut snapshots=migrations["snapshots"].as_object().unwrap().clone();assert!(snapshots.insert("selection".into(),selection["snapshot"].clone()).is_none());
    let receipt=json!({"schema_version":2,"kind":"source-selection-schema5-runtime/v2","nonce":nonce,"source_tree_sha256":source,"artifact_sha256":artifact_sha,"environment":fixture_environment(),"started_at_ms":migrations["started_at_ms"],"finished_at_ms":wall_ms(),"producer":{"package":"app-infra","target":"source_selection_sets","tests":["bj_08_storage","bj_09_storage","bj_11_storage","bj_12_storage","bj_17_storage"],"source_sha256":content_hash(include_bytes!("source_selection_sets.rs")),"binary":root_artifact(&executable)},"snapshots":snapshots,"routes":migrations["routes"],"field_facts":facts["fields"],"relationship_facts":facts["relationships_and_triggers"],"sqlite":migrations["sqlite"],"host":{"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,"os_release":String::from_utf8(release.stdout).unwrap().trim()},"route_trace":migrations["route_trace"],"old_writer":old_ref(".build/binding-journey-alignment-20261109/frozen-store4/reader-manifest.json"),"old_suite":{"schema":old_ref(".build/binding-journey-alignment-20261109/frozen-store4/original-schema-suite/result.json"),"dependent":old_ref(".build/binding-journey-alignment-20261109/frozen-store4/original-dependent-suites-corrected-v3/results.json"),"lists":old_ref(".build/binding-journey-alignment-20261109/frozen-store4/original-suite-test-lists/inventory.json")},"selection_snapshot":"selection","selection_rows":selection["rows"],"child_proof":root_artifact(&child_path),"production_timelines":timelines});write_new(&base.join("storage-v5-runtime.json"),&receipt);
}

#[test]
fn fresh_original_journal_rejects_precreated_and_replaced_database_files() {
    let precreated=directory("fresh-precreated-negative").join("empty.sqlite3");std::fs::write(&precreated,[]).unwrap();
    assert!(std::panic::catch_unwind(||SqliteStore::observe_schema5_fresh_open(&precreated,"fresh_original_journal_rejects_precreated_and_replaced_database_files","fresh-negative","fresh-negative")).is_err());
    let rejected=match SqliteStore::open(&precreated,"fresh-negative","fresh-negative",1){Ok(_)=>panic!("actual empty precreation must not take fresh route"),Err(error)=>error};assert_eq!(rejected.code,"unsupported_schema");assert!(std::fs::read(&precreated).unwrap().is_empty());assert!(!precreated.parent().unwrap().join("fresh-original-callbacks.jsonl").exists());
    let uncreated=directory("fresh-precreation-error-negative").join("never-created.sqlite3");let incomplete=SqliteStore::observe_schema5_fresh_open(&uncreated,"fresh_original_journal_rejects_precreated_and_replaced_database_files","fresh-negative","fresh-negative");let failed=match SqliteStore::open(&uncreated,"fresh-negative","fresh-negative",-1){Ok(_)=>panic!("invalid-time API call must fail before creating a file"),Err(error)=>error};assert_eq!(failed.code,"invalid_time");assert!(!uncreated.exists());assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||incomplete.seal_fresh(json!({})))).is_err(),"precreation failure has no positive artifact identity and cannot be sealed");assert!(!uncreated.parent().unwrap().join("fresh-sealed-timeline.json").exists());assert!(!uncreated.parent().unwrap().join("fresh-first-observed-identity.json").exists());drop(incomplete);
    let original=directory("fresh-replaced-negative").join("original.sqlite3");let guard=SqliteStore::observe_schema5_fresh_open(&original,"fresh_original_journal_rejects_precreated_and_replaced_database_files","fresh-negative","fresh-negative");drop(SqliteStore::open(&original,"fresh-negative","fresh-negative",1).unwrap());let first=file_identity(&original);let retained=original.with_file_name("actual-original-retained.sqlite3");std::fs::rename(&original,&retained).unwrap();std::fs::copy(&retained,&original).unwrap();assert_ne!(file_identity(&original),first,"negative must actually replace the original inode even with identical bytes");
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||guard.seal_fresh(json!({})))).is_err());std::fs::remove_file(&original).unwrap();std::fs::rename(&retained,&original).unwrap();assert_eq!(file_identity(&original),first);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(||guard.seal_fresh(json!({})))).is_err(),"a previously observed replacement must stay invalidated even if the first inode returns");assert!(!original.parent().unwrap().join("fresh-sealed-timeline.json").exists());assert!(!original.parent().unwrap().join("fresh-raw-closure.json").exists());drop(guard);
}

#[test]
fn fresh_original_callback_journal_records_missing_before_real_creation() {
    let out=directory("fresh-positive-journal");let path=out.join("original.sqlite3");assert!(!path.exists());
    let guard=SqliteStore::observe_schema5_fresh_open(&path,"fresh_original_callback_journal_records_missing_before_real_creation","fresh-positive","fresh-positive");let store=SqliteStore::open(&path,"fresh-positive","fresh-positive",100).unwrap();guard.control.emit("caller_received_success","caller",json!({}));store.checkpoint().unwrap();drop(store);inspect(&path,5);
    let source_identity_before=file_identity(&path);let snapshot=out.join("sealed.sqlite3");let db=Sqlite::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();db.backup("main",&snapshot,None).unwrap();drop(db);inspect(&snapshot,5);let source_identity_after=file_identity(&path);assert_eq!(source_identity_before,source_identity_after);let capture=json!({"source_path":path,"source_identity_before":source_identity_before,"source_identity_after":source_identity_after,"snapshot_base":root(),"snapshot":root_artifact(&snapshot),"captured_at_ms":wall_ms()});guard.control.emit("independent_current5_db_ledger_invariants","independent_readback",json!({"before":null,"after":root_artifact(&snapshot),"backup":null}));let proof=guard.seal_fresh(capture);
    let raw=std::fs::read_to_string(out.join("fresh-original-callbacks.jsonl")).unwrap();let raw=raw.lines().map(|line|serde_json::from_str::<Value>(line).unwrap()["callback"].clone()).collect::<Vec<_>>();let first:Value=serde_json::from_slice(&std::fs::read(out.join("fresh-first-observed-identity.json")).unwrap()).unwrap();
    for kind in ["api_entry","database_writer_entry"]{let event=raw.iter().find(|e|e["event_kind"]==kind).unwrap();assert!(event.get("database_file_device_identity").is_none());assert_eq!(event["payload"]["filesystem_observation"]["file_missing"],true);assert!(event["monotonic_ns"].as_u64().unwrap()<=first["monotonic_ns"].as_u64().unwrap());assert!(event["event_sequence"].as_u64().unwrap()<=first["after_callback_sequence"].as_u64().unwrap());}
    assert_eq!(first["file_identity"],file_identity(&path));assert_eq!(first["file_identity"],proof["snapshot_capture"]["source_identity_after"]);assert_ne!(first["file_identity"],file_identity(&snapshot),"the original identity must not be borrowed from its independently captured snapshot");assert!(proof["seal_observation"]["monotonic_ns"].as_u64().unwrap()>=first["monotonic_ns"].as_u64().unwrap());
}
