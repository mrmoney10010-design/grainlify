# Grainlify Core Storage Layout

This document defines the storage layout for the `grainlify-core` contract. Any modifications to structural types or addition of keys must be reflected here.

## Storage Schema Version: 1

Below are all storage keys utilized by the contract.

| Key | Variant/Constant | Tier | Type | Notes |
|-----|-----------------|------|------|-------|
| `DataKey::Admin` | `Admin` | Instance | `Address` | Set once at initialization |
| `DataKey::Version` | `Version` | Instance | `u32` | Current contract version |
| `DataKey::PreviousVersion` | `PreviousVersion` | Instance | `u32` | Version before the last upgrade |
| `DataKey::MigrationState` | `MigrationState` | Instance | `MigrationState` | Double-migration guard |
| `DataKey::UpgradeProposal(u64)` | `UpgradeProposal(id)` | Instance | `BytesN<32>` | Per-proposal wasm hash |
| `DataKey::ConfigSnapshot(u64)` | `ConfigSnapshot(id)` | Instance | `CoreConfigSnapshot` | Snapshotted configuration |
| `DataKey::SnapshotIndex` | `SnapshotIndex` | Instance | `Vec<u64>` | Ordered snapshot id list |
| `DataKey::SnapshotCounter` | `SnapshotCounter` | Instance | `u64` | Monotone counter |
| `DataKey::ChainId` | `ChainId` | Instance | `String` | Cross-network protection |
| `DataKey::NetworkId` | `NetworkId` | Instance | `String` | Environment selector |
| `DataKey::ReadOnlyMode` | `ReadOnlyMode` | Instance | `bool` | Blocks state-mutating operations |
| `"op_count"` | (Symbol) | Persistent | `u64` | Monitoring operations counter |
| `"usr_count"` | (Symbol) | Persistent | `u64` | Monitoring unique users |
| `"err_count"` | (Symbol) | Persistent | `u64` | Monitoring error counter |
| `("perf_cnt", Symbol)` | Tuple | Persistent | `u64` | Hit count per-function |
| `("perf_time", Symbol)` | Tuple | Persistent | `u64` | Cumulative duration per-function |

## Migration Steps
If modifying the schema:
1. Bump `STORAGE_SCHEMA_VERSION`.
2. Update this layout document.
3. Write `migrate` implementations that gracefully handle reading old variants and overwriting them with new variants.
4. Update `verify_storage_layout()` assertions to reflect the new requirements.
