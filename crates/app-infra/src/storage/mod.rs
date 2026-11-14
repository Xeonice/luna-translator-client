//! Dedicated bounded SQLite writer. Product migrations are embedded from this module, never docs.
mod database;
mod queries;
pub use database::{
    PROFILE_SCHEMA_VERSION, SqliteStore, create_dictionary, http_service_identity_migration_sha256,
    profile_migration_sha256, session_records_migration_sha256, service_graph_migration_sha256,
};

pub use queries::offline_resources::{
    LiveOfflinePreparations, OfflinePreparationGuard, SqliteOfflineReadCheckpoint, SqliteOfflineResources,
    OfflineEnvironmentMetadata, OfflineLocalLocation, OfflineMaterialLookup, OfflineResourcePair,
    OfflineSnapshotIdentity, OfflineStoredPreparationReceipt, OfflineUnknownMetadata,
    OfflineUnknownPreparation, OfflineRetentionReport, offline_route_content_hash, offline_snapshot_content_hash,
};
