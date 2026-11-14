//! Real filesystem and SQLite writer acceptance; the controlled codec isolates storage from native decoding.
use app_core::{
    contracts::*,
    domain::{capture::*, content_hash},
    ports::{GameCoverAssets, ProductRepository},
};
use app_infra::{platform::game_assets::*, storage::SqliteStore};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

struct Codec;
struct OutOfContractCodec {
    length: usize,
    width: u32,
    height: u32,
}
impl CoverImageCodec for OutOfContractCodec {
    fn normalize(&self, _: &[u8]) -> AppResult<NormalizedCover> {
        Ok(NormalizedCover {
            bytes: vec![7; self.length],
            width: self.width,
            height: self.height,
        })
    }
    fn dimensions(&self, _: &[u8]) -> AppResult<(u32, u32)> {
        Ok((self.width, self.height))
    }
}
#[test]
fn managed_store_rejects_codec_output_outside_all_frozen_bounds_before_writing() {
    for (length, width, height) in [
        (0, 600, 400),
        (COVER_LIMIT + 1, 600, 400),
        (100, 159, 400),
        (100, 600, 89),
        (100, 961, 400),
        (100, 600, 961),
    ] {
        let root = directory();
        let assets = ManagedGameAssets::new(
            root.clone(),
            Arc::new(OutOfContractCodec {
                length,
                width,
                height,
            }),
        )
        .unwrap();
        assert_eq!(
            assets
                .import_session_frame(b"controlled-input")
                .unwrap_err()
                .code,
            "cover_image_bounds"
        );
        assert_eq!(
            std::fs::read_dir(root.join("assets/covers/v1"))
                .unwrap()
                .count(),
            0
        );
    }
}
#[test]
fn sparse_real_file_enforces_total_quota_and_exact_per_asset_limit_without_overwriting() {
    let root = directory();
    let assets = ManagedGameAssets::new(
        root.clone(),
        Arc::new(OutOfContractCodec {
            length: COVER_LIMIT,
            width: 960,
            height: 960,
        }),
    )
    .unwrap();
    let occupied = root.join("assets/covers/v1/quota-reservation.jpg");
    let file = std::fs::File::create(&occupied).unwrap();
    file.set_len(256 * 1024 * 1024).unwrap();
    assert_eq!(
        assets
            .import_session_frame(b"controlled-input")
            .unwrap_err()
            .code,
        "cover_store_limit"
    );
    assert_eq!(
        std::fs::read_dir(root.join("assets/covers/v1"))
            .unwrap()
            .count(),
        1
    );
    file.set_len(256 * 1024 * 1024 - COVER_LIMIT as u64)
        .unwrap();
    let cover = assets.import_session_frame(b"controlled-input").unwrap();
    assert_eq!(
        assets.read_cover(&cover.asset_ref).unwrap().bytes.len(),
        COVER_LIMIT
    );
    assert_eq!(
        assets
            .import_session_frame(b"same-normalized-content")
            .unwrap()
            .asset_ref,
        cover.asset_ref
    );
    assert_eq!(
        std::fs::read_dir(root.join("assets/covers/v1"))
            .unwrap()
            .count(),
        2
    );
}
impl CoverImageCodec for Codec {
    fn normalize(&self, source: &[u8]) -> AppResult<NormalizedCover> {
        if !source.starts_with(b"controlled-image:") {
            return Err(AppError::new(
                "cover_image_invalid",
                "test",
                "controlled invalid image",
            ));
        }
        Ok(NormalizedCover {
            bytes: source.to_vec(),
            width: 600,
            height: 900,
        })
    }
    fn dimensions(&self, source: &[u8]) -> AppResult<(u32, u32)> {
        self.normalize(source)
            .map(|image| (image.width, image.height))
    }
}
fn directory() -> PathBuf {
    let root = std::env::temp_dir().join(format!("luna-cover-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
}
fn game(exe: &Path) -> Game {
    Game {
        game_id: "cover-game".into(),
        display_name: "台詞のゲーム".into(),
        original_title: None,
        source_language: Some("ja".into()),
        cover_asset_ref: None,
        row_revision: 1,
        target: GameTarget {
            environment_kind: "unresolved".into(),
            environment_key: "unresolved:cover".into(),
            bottle_locator: None,
            launch_path: Some(exe.to_string_lossy().into()),
            canonical_exe_path: None,
            target_path_key: None,
            working_directory: None,
            arguments: vec![],
            architecture: Some("x86".into()),
            verification_state: "unresolved".into(),
        },
    }
}
fn executable(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes = vec![0; 88];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
    bytes[64..68].copy_from_slice(b"PE\0\0");
    bytes[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}
#[test]
fn new_and_old_steam_layouts_and_localized_portrait_resolve_by_manifest_not_name() {
    for old in [false, true] {
        let root = directory();
        let exe = root.join("Steam/steamapps/common/正規のディレクトリ/Game.exe");
        executable(&exe);
        let steam = root.join("Steam");
        std::fs::write(steam.join("steamapps/appmanifest_987654.acf"), "\"AppState\" { \"appid\" \"987654\" \"installdir\" \"正規のディレクトリ\" \"InstalledDepots\" { \"appid\" \"wrong\" } }").unwrap();
        let cache = steam.join("appcache/librarycache");
        std::fs::create_dir_all(cache.join("987654")).unwrap();
        let image = if old {
            cache.join("987654_library_600x900.jpg")
        } else {
            cache.join("987654/library_600x900_schinese.jpg")
        };
        std::fs::write(image, b"controlled-image:portrait").unwrap();
        let assets = ManagedGameAssets::new(root.join("profile"), Arc::new(Codec)).unwrap();
        let mut input = game(&exe);
        input.display_name = "Unrelated title, not an appid mapping".into();
        let cover = assets.import_local_cover(&input).unwrap().unwrap();
        assert_eq!(cover.origin, GameCoverOrigin::SteamLibrary);
        let read = assets.read_cover(&cover.asset_ref).unwrap();
        assert_eq!(read.bytes, b"controlled-image:portrait");
        assert_eq!(content_hash(&read.bytes), read.prepared.content_hash);
        assert_eq!(
            std::fs::read_dir(root.join("profile/assets/covers/v1"))
                .unwrap()
                .count(),
            1
        );
    }
}
#[test]
fn invalid_or_missing_local_art_does_not_save_candidate_frames_and_session_frame_is_explicit() {
    let root = directory();
    let exe = root.join("game/Game.exe");
    executable(&exe);
    std::fs::write(root.join("game/cover.jpg"), b"<html>not an image</html>").unwrap();
    let assets = ManagedGameAssets::new(root.join("profile"), Arc::new(Codec)).unwrap();
    assert!(assets.import_local_cover(&game(&exe)).unwrap().is_none());
    assert_eq!(
        std::fs::read_dir(root.join("profile/assets/covers/v1"))
            .unwrap()
            .count(),
        0
    );
    let cover = assets
        .import_session_frame(b"controlled-image:trusted-session")
        .unwrap();
    assert_eq!(cover.origin, GameCoverOrigin::SessionFrame);
    assert!(
        assets
            .import_session_frame(&vec![1; COVER_LIMIT + 1])
            .is_err()
    );
    assert_eq!(
        std::fs::read_dir(root.join("profile/assets/covers/v1"))
            .unwrap()
            .count(),
        1
    );
}
#[test]
fn opaque_references_reject_paths_urls_unknown_origin_and_hash_tampering() {
    for bad in [
        "../secret",
        "file:///tmp/x.jpg",
        "https://host/x",
        "local-cover/旧图.png",
        "cover:v1:session_frame:../x",
        "cover:v1:other:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(parse_reference(bad).is_err(), "{bad}");
    }
    let root = directory();
    let assets = ManagedGameAssets::new(root.clone(), Arc::new(Codec)).unwrap();
    let cover = assets
        .import_session_frame(b"controlled-image:one")
        .unwrap();
    std::fs::write(
        root.join("assets/covers/v1")
            .join(format!("{}.jpg", cover.content_hash)),
        b"controlled-image:tampered",
    )
    .unwrap();
    assert_eq!(
        assets.read_cover(&cover.asset_ref).unwrap_err().code,
        "cover_hash_mismatch"
    );
}
#[cfg(unix)]
#[test]
fn source_symlink_and_managed_directory_symlink_cannot_escape_profile_roots() {
    let root = directory();
    let exe = root.join("game/Game.exe");
    executable(&exe);
    let outside = directory().join("private.jpg");
    std::fs::write(&outside, b"controlled-image:private").unwrap();
    std::os::unix::fs::symlink(outside, root.join("game/cover.jpg")).unwrap();
    let assets = ManagedGameAssets::new(root.join("profile"), Arc::new(Codec)).unwrap();
    assert!(assets.import_local_cover(&game(&exe)).unwrap().is_none());
    let other = directory();
    std::os::unix::fs::symlink(&other, root.join("bad-profile")).unwrap();
    assert!(ManagedGameAssets::new(root.join("bad-profile"), Arc::new(Codec)).is_err());
    std::fs::create_dir_all(root.join("linked-profile")).unwrap();
    std::os::unix::fs::symlink(other, root.join("linked-profile/assets")).unwrap();
    assert!(ManagedGameAssets::new(root.join("linked-profile"), Arc::new(Codec)).is_err());
}
#[test]
fn malformed_ambiguous_and_oversized_manifest_fields_are_rejected() {
    for bad in [
        "\"AppState\" { \"appid\" \"1\" \"appid\" \"2\" }",
        "\"AppState\" { \"appid\" \"unterminated",
        "\"AppState\" { \"appid\" }",
        "\"AppState\" {} extra",
        "\"Other\" { \"appid\" \"1\" }",
    ] {
        assert!(manifest_fields(bad.as_bytes()).is_err(), "{bad}");
    }
    assert!(manifest_fields(&vec![b' '; 65_537]).is_err());
    let parsed=manifest_fields(b"//comment\n\"AppState\" { \"appid\" \"1\" \"installdir\" \"a b\" \"nested\" { \"appid\" \"999\" }}").unwrap();
    assert_eq!(parsed["appid"], "1");
    assert_eq!(parsed["installdir"], "a b");
}
struct Db {
    store: SqliteStore,
    root: PathBuf,
    id: String,
}
impl Db {
    fn new() -> Self {
        let root = directory();
        let store = SqliteStore::open(
            root.join("reader.sqlite3"),
            "cover-profile",
            "cover-server",
            1,
        )
        .unwrap();
        let input = GameInput {
            game_id: None,
            row_revision: None,
            display_name: "日本語 title".into(),
            original_title: Some("original".into()),
            source_language: Some("ja".into()),
            cover_asset_ref: None,
            target: GameTarget {
                environment_kind: "crossover".into(),
                environment_key: "crossover:/bottles/cover".into(),
                bottle_locator: Some("/bottles/cover".into()),
                launch_path: Some("C:\\Games\\Game.exe".into()),
                canonical_exe_path: Some("C:\\Games\\Game.exe".into()),
                target_path_key: None,
                working_directory: None,
                arguments: vec!["keep".into()],
                architecture: Some("x86".into()),
                verification_state: "verified".into(),
            },
        };
        let id = store
            .dispatch(
                CommandEnvelope {
                    operation_id: "create".into(),
                    expected_revision: None,
                    command: Command::SaveGame(Box::new(input)),
                },
                10,
            )
            .unwrap()
            .result_id
            .unwrap();
        Self { store, root, id }
    }
    fn guard(&self, session: Option<CoverSessionGuard>) -> CoverCommitGuard {
        let s = self.store.snapshot().unwrap();
        let g = &s.games[0];
        CoverCommitGuard {
            profile_id: s.profile_id,
            profile_generation: s.profile_generation,
            server_instance_id: s.server_instance_id,
            game_id: g.game_id.clone(),
            row_revision: g.row_revision,
            environment_key: g.target.environment_key.clone(),
            target_path_key: g.target.target_path_key.clone(),
            session,
        }
    }
    fn open(&self, id: &str) -> CoverSessionGuard {
        self.store
            .open_hook_session(
                HookSessionInput {
                    session_id: id.into(),
                    game_id: Some(self.id.clone()),
                    environment_key: "crossover:/bottles/cover".into(),
                    target_exe_path: "C:\\Games\\Game.exe".into(),
                    windows_pid: 42,
                    process_created_u64: "123456".into(),
                    architecture: "x86".into(),
                    broker_instance_id: format!("broker-{id}"),
                    resource: ResourceSnapshotInput {
                        snapshot_id: "cover-core".into(),
                        kind: "core".into(),
                        version: "test".into(),
                        manifest_sha256: content_hash("resource"),
                        contract_version: "abi1-jsonl2".into(),
                        metadata_json: "{}".into(),
                    },
                    target_identity_sha256: content_hash(id),
                },
                20,
            )
            .unwrap();
        CoverSessionGuard {
            session_id: id.into(),
            process_identity_sha256: content_hash(id),
            binding_identity_sha256: content_hash(format!("window:{id}")),
        }
    }
}
fn prepared(label: &str) -> PreparedGameCover {
    let hash = content_hash(label);
    PreparedGameCover {
        asset_ref: format!("cover:v1:session_frame:{hash}"),
        origin: GameCoverOrigin::SessionFrame,
        content_hash: hash,
        width: 600,
        height: 400,
    }
}
#[test]
fn real_writer_cas_changes_only_cover_and_revision_and_never_overwrites_existing_art() {
    let f = Db::new();
    let before = f.store.snapshot().unwrap();
    let frozen = f.guard(None);
    assert_eq!(
        f.store
            .set_game_cover_if_empty(frozen.clone(), prepared("one"), 30)
            .unwrap(),
        CoverInstallOutcome::Installed
    );
    assert_eq!(
        f.store
            .set_game_cover_if_empty(frozen, prepared("two"), 31)
            .unwrap(),
        CoverInstallOutcome::AlreadyPresent
    );
    let after = f.store.snapshot().unwrap();
    assert_eq!(after.commit_revision, before.commit_revision + 1);
    assert_eq!(
        after.games[0].row_revision,
        before.games[0].row_revision + 1
    );
    let mut expected = before.games[0].clone();
    expected.row_revision += 1;
    expected.cover_asset_ref = Some(prepared("one").asset_ref);
    assert_eq!(after.games[0], expected);
    let conn = rusqlite::Connection::open(f.root.join("reader.sqlite3")).unwrap();
    let fk: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(fk, 0);
}
#[test]
fn real_writer_rejects_every_identity_boundary_without_mutating_game() {
    let f = Db::new();
    let before = f.store.snapshot().unwrap();
    let base = f.guard(None);
    for (mutated, expected) in [
        (
            {
                let mut x = base.clone();
                x.row_revision += 1;
                x
            },
            CoverInstallOutcome::RevisionConflict,
        ),
        (
            {
                let mut x = base.clone();
                x.environment_key.push('x');
                x
            },
            CoverInstallOutcome::TargetChanged,
        ),
        (
            {
                let mut x = base.clone();
                x.target_path_key = Some("other".into());
                x
            },
            CoverInstallOutcome::TargetChanged,
        ),
        (
            {
                let mut x = base.clone();
                x.server_instance_id.push('x');
                x
            },
            CoverInstallOutcome::StaleSession,
        ),
        (
            {
                let mut x = base.clone();
                x.profile_generation += 1;
                x
            },
            CoverInstallOutcome::StaleSession,
        ),
        (
            {
                let mut x = base.clone();
                x.profile_id.push('x');
                x
            },
            CoverInstallOutcome::StaleSession,
        ),
        (
            {
                let mut x = base.clone();
                x.game_id = "missing".into();
                x
            },
            CoverInstallOutcome::Archived,
        ),
    ] {
        assert_eq!(
            f.store
                .set_game_cover_if_empty(mutated, prepared("bad"), 30)
                .unwrap(),
            expected
        );
    }
    assert_eq!(f.store.snapshot().unwrap(), before);
    f.store
        .dispatch(
            CommandEnvelope {
                operation_id: "archive".into(),
                expected_revision: None,
                command: Command::ArchiveGame {
                    game_id: f.id.clone(),
                    row_revision: 1,
                },
            },
            31,
        )
        .unwrap();
    assert_eq!(
        f.store
            .set_game_cover_if_empty(base, prepared("bad"), 32)
            .unwrap(),
        CoverInstallOutcome::Archived
    );
}
#[test]
fn session_start_revision_and_old_process_identity_cannot_commit_a_late_frame() {
    let f = Db::new();
    let pre_association = f.guard(None);
    let session = f.open("first");
    let post = f.guard(Some(session.clone()));
    assert_eq!(post.row_revision, pre_association.row_revision + 1);
    assert_eq!(
        f.store
            .set_game_cover_if_empty(
                CoverCommitGuard {
                    session: Some(session.clone()),
                    ..pre_association
                },
                prepared("old-revision"),
                30
            )
            .unwrap(),
        CoverInstallOutcome::RevisionConflict
    );
    let wrong_process = CoverSessionGuard {
        process_identity_sha256: content_hash("wrong-process"),
        ..session
    };
    assert_eq!(
        f.store
            .set_game_cover_if_empty(
                CoverCommitGuard {
                    session: Some(wrong_process),
                    ..post.clone()
                },
                prepared("wrong-process"),
                31
            )
            .unwrap(),
        CoverInstallOutcome::StaleSession
    );
    f.open("second");
    let mut late = f.guard(post.session);
    late.row_revision = f.store.snapshot().unwrap().games[0].row_revision;
    assert_eq!(
        f.store
            .set_game_cover_if_empty(late, prepared("late-first"), 32)
            .unwrap(),
        CoverInstallOutcome::StaleSession
    );
    assert!(
        f.store.snapshot().unwrap().games[0]
            .cover_asset_ref
            .is_none()
    );
}
#[test]
fn competing_real_writer_commits_install_exactly_one_cover() {
    let f = Db::new();
    let frozen = f.guard(None);
    let a = f.store.clone();
    let b = f.store.clone();
    let one = frozen.clone();
    let t1 = std::thread::spawn(move || {
        a.set_game_cover_if_empty(one, prepared("first"), 30)
            .unwrap()
    });
    let t2 = std::thread::spawn(move || {
        b.set_game_cover_if_empty(frozen, prepared("second"), 31)
            .unwrap()
    });
    let results = [t1.join().unwrap(), t2.join().unwrap()];
    assert_eq!(
        results
            .iter()
            .filter(|&&r| r == CoverInstallOutcome::Installed)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|&&r| r == CoverInstallOutcome::AlreadyPresent)
            .count(),
        1
    );
    assert_eq!(f.store.snapshot().unwrap().games[0].row_revision, 2);
}

#[test]
fn real_session_null_mapping_supplement_enables_only_its_own_first_frame_cover() {
    let f = Db::new();
    let mut input = HookSessionInput { session_id: "mapping-first-frame".into(), game_id: None, environment_key: "crossover:/bottles/cover".into(), target_exe_path: "C:\\Games\\Game.exe".into(), windows_pid: 42, process_created_u64: "123456".into(), architecture: "x86".into(), broker_instance_id: "mapping-broker".into(), resource: ResourceSnapshotInput { snapshot_id: "mapping-core".into(), kind: "core".into(), version: "test".into(), manifest_sha256: content_hash("resource"), contract_version: "abi1-jsonl2".into(), metadata_json: "{}".into() }, target_identity_sha256: content_hash("mapping-first-frame") };
    f.store.open_hook_session(input.clone(), 20).unwrap();
    let session = CoverSessionGuard { session_id: input.session_id.clone(), process_identity_sha256: input.target_identity_sha256.clone(), binding_identity_sha256: content_hash("mapping-window") };
    assert_eq!(f.store.set_game_cover_if_empty(f.guard(Some(session.clone())), prepared("unmapped-frame"), 21).unwrap(), CoverInstallOutcome::StaleSession);
    assert!(f.store.snapshot().unwrap().games[0].cover_asset_ref.is_none());
    input.game_id = Some(f.id.clone());
    f.store.open_hook_session(input.clone(), 22).unwrap();
    let after = f.store.snapshot().unwrap();
    assert_eq!(after.session.as_ref().unwrap().session_id, input.session_id);
    assert_eq!(after.session.as_ref().unwrap().game_id, Some(f.id.clone()));
    assert_eq!(f.store.set_game_cover_if_empty(f.guard(Some(session)), prepared("mapped-frame"), 23).unwrap(), CoverInstallOutcome::Installed);
    assert_eq!(f.store.snapshot().unwrap().games[0].cover_asset_ref, Some(prepared("mapped-frame").asset_ref));
    let mut wrong = input.clone(); wrong.game_id = Some("not-this-game".into());
    assert_eq!(f.store.open_hook_session(wrong, 24).unwrap_err().code, "session_game_conflict");
    assert_eq!(f.store.snapshot().unwrap().session.unwrap().game_id, input.game_id);
}
