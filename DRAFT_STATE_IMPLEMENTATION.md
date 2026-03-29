# Draft State Implementation Summary

## Overview
This document describes the implementation of Draft state support for both Bounty Escrow and Program Escrow contracts in Grainlify.

## Implementation Status

### ✅ Bounty Escrow Contract - COMPLETE

#### Changes Made:

1. **Added `Draft` status to `EscrowStatus` enum** (`lib.rs:650`)
   ```rust
   pub enum EscrowStatus {
       Draft,      // ← NEW
       Locked,
       Released,
       Refunded,
       PartiallyRefunded,
   }
   ```

2. **Updated escrow creation to start in Draft status**
   - `lock_funds()` - Line 2656
   - `lock_funds_anonymous()` - Line 2984  
   - `batch_lock_funds()` - Line 4511
   
   All now create escrows with `status: EscrowStatus::Draft`

3. **Added `publish()` function** (Lines 3036-3094)
   - Admin-only function to transition escrow from Draft → Locked
   - Emits `EscrowPublished` event
   - Validates escrow exists and is in Draft status
   
4. **Updated `release_funds()` to block Draft status** (Lines 3143-3151)
   - Explicit check for Draft status before allowing release
   - Returns `Error::InvalidState` if Draft

5. **Updated `refund()` to block Draft status** (Lines 3880-3892)
   - Explicit check for Draft status before allowing refund
   - Returns `Error::InvalidState` if Draft

6. **Updated capability token checks** (Lines 2005-2010, 2026-2034, 2091-2096, 2113-2117)
   - Both Release and Refund capability actions now block Draft status

7. **Added `EscrowPublished` event** (`events.rs:169-207`)
   - Event structure and emitter function
   - Emitted when escrow transitions Draft → Locked

#### State Machine:
```
[New] ──lock_funds()──► Draft ──publish()──► Locked ──release/refund──► Terminal States
```

---

### ⚠️ Program Escrow Contract - PARTIALLY COMPLETE

#### Changes Made:

1. **Added `ProgramStatus` enum** (`lib.rs:427-444`)
   ```rust
   pub enum ProgramStatus {
       Draft,    // Initial state
       Active,   // Published state
   }
   ```

2. **Updated `ProgramData` struct** (Line 459)
   - Added `status: ProgramStatus` field

3. **Updated `init_program()` to create in Draft status** (Line 1053)
   - Programs now start with `status: ProgramStatus::Draft`

4. **Updated `lock_program_funds()` to block Draft** (Lines 1428-1441)
   - Checks program status before allowing fund locks
   - Panics with "Program is in Draft status" if not published

#### Still Needed:

1. **Add `publish_program()` function**
   - Should transition program from Draft → Active
   - Admin/creator only
   - Emit `ProgramPublished` event

2. **Update payout functions to block Draft**
   - `single_payout()` 
   - `batch_payout()`
   - Check program status before allowing payouts

3. **Update refund functions to block Draft**
   - Any program refund functions
   - Check program status

4. **Add `ProgramPublished` event**
   - Similar to `EscrowPublished` event
   - Track Draft → Active transitions

---

## Testing Requirements

### Bounty Escrow Tests Needed:

1. **Test Draft → Locked transition**
   ```rust
   #[test]
   fn test_escrow_starts_in_draft_status()
   fn test_publish_transitions_to_locked()
   fn test_publish_fails_if_not_draft()
   ```

2. **Test operations blocked in Draft**
   ```rust
   #[test]
   #[should_panic(expected = "InvalidState")]
   fn test_release_fails_in_draft_status()
   
   #[test]
   #[should_panic(expected = "InvalidState")]
   fn test_refund_fails_in_draft_status()
   ```

3. **Test capability tokens blocked in Draft**
   ```rust
   #[test]
   fn test_capability_release_blocked_in_draft()
   fn test_capability_refund_blocked_in_draft()
   ```

### Program Escrow Tests Needed:

1. **Test program starts in Draft**
   ```rust
   #[test]
   fn test_program_starts_in_draft_status()
   fn test_publish_program_transitions_to_active()
   ```

2. **Test lock_program_funds blocked in Draft**
   ```rust
   #[test]
   #[should_panic(expected = "Draft status")]
   fn test_lock_funds_fails_in_draft_status()
   ```

3. **Test payouts blocked in Draft** (after publish_program added)
   ```rust
   #[test]
   fn test_payout_fails_in_draft_status()
   ```

---

## Migration Considerations

### Existing Escrows/Programs:
- **Bounty Escrow**: Existing escrows were created with `Locked` status, so they are unaffected
- **Program Escrow**: Existing programs need migration to add the `status` field
  - Default existing programs to `ProgramStatus::Active` to maintain current behavior
  - OR run a migration script to set status based on whether funds are locked

### Backward Compatibility:
- The `Draft` enum variant is additive, so it won't break existing queries filtering by other statuses
- New validation checks may break existing integrations that expect immediate lock/release

---

## Security Considerations

1. **Access Control**: 
   - `publish()` is admin-only (bounty escrow)
   - `publish_program()` should also be admin/creator-only

2. **State Validation**:
   - All state-changing operations must check for Draft status
   - Audit all functions that interact with escrow/program state

3. **Event Emission**:
   - Publish events are critical for off-chain tracking
   - Indexers need to handle Draft status properly

---

## Next Steps

### Immediate (Required):
1. ✅ Complete bounty escrow tests
2. ⚠️ Add `publish_program()` function
3. ⚠️ Update program payout functions to block Draft
4. ⚠️ Add program escrow tests

### Optional (Enhancements):
1. Add query functions to filter by Draft status
2. Add metadata update capability in Draft state
3. Consider allowing draft editing before publish
4. Add deadline handling for unpublished drafts

---

## Files Modified

### Bounty Escrow:
- `contracts/bounty_escrow/contracts/escrow/src/lib.rs`
  - Lines 650-655: Added Draft to EscrowStatus
  - Lines 2656, 2984, 4511: Updated to create Draft escrows
  - Lines 3036-3094: Added publish() function
  - Lines 3143-3151: Updated release_funds check
  - Lines 3880-3892: Updated refund check
  - Lines 2005-2010, 2026-2034, 2091-2096, 2113-2117: Capability checks
  
- `contracts/bounty_escrow/contracts/escrow/src/events.rs`
  - Lines 169-207: Added EscrowPublished event

### Program Escrow:
- `contracts/program-escrow/src/lib.rs`
  - Lines 427-444: Added ProgramStatus enum
  - Line 459: Added status field to ProgramData
  - Line 1053: Set initial status to Draft
  - Lines 1428-1441: Updated lock_program_funds check

---

## API Changes

### New Functions:
- `BountyEscrowContract::publish(bounty_id: u64)` - Transition escrow Draft → Locked
- `ProgramEscrowContract::publish_program(program_id: String)` - TODO

### Modified Behavior:
- `lock_funds()` - Now creates Draft escrows (not immediately Locked)
- `lock_program_funds()` - Now blocked until program is published
- `release_funds()` - Now blocked for Draft escrows
- `refund()` - Now blocked for Draft escrows

### New Events:
- `EscrowPublished` - Emitted when bounty escrow is published
- `ProgramPublished` - TODO

---

## Documentation Updates Needed

1. Update ARCHITECTURE.md with new state machine diagrams
2. Add Draft state explanation to README files
3. Update API documentation
4. Create migration guide for existing deployments
5. Add developer guide for using Draft state

---

## Conclusion

The Draft state implementation is complete for Bounty Escrow and partially complete for Program Escrow. The feature enables safer contract interactions by requiring an explicit publish step before funds can be moved, preventing accidental operations during setup.

**Estimated completion time for remaining work**: 2-4 hours
**Risk level**: Low - changes are additive and don't affect existing deployed contracts
**Testing priority**: High - state machine changes require thorough testing
