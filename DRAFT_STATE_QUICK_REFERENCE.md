# Draft State - Quick Reference

## For Developers

### Bounty Escrow Usage

```rust
// Step 1: Create escrow in Draft state
lock_funds(depositor, bounty_id, amount, deadline);
// Status: Draft (funds locked but frozen)

// Step 2: Review period (optional)
// No actions possible on Draft escrow

// Step 3: Publish to activate
publish(bounty_id);
// Status: Locked (normal operations enabled)

// Step 4: Normal operations
release_funds(bounty_id, contributor);
// OR
refund(bounty_id);
```

### Key Functions

| Function | Behavior Change |
|----------|----------------|
| `lock_funds()` | Now creates **Draft** escrows |
| `publish()` | **NEW** - Transitions Draft → Locked |
| `release_funds()` | Blocked for Draft status |
| `refund()` | Blocked for Draft status |

### Error Codes

| Error | When |
|-------|------|
| `InvalidState` | Trying to release/refund Draft escrow |
| `InvalidState` | Publishing non-Draft escrow |
| `BountyNotFound` | Publishing non-existent bounty |

---

## For Frontend Developers

### UI States

```typescript
enum EscrowStatus {
  Draft = 0,           // "Pending Publication"
  Locked = 1,          // "Active"
  Released = 2,        // "Released"
  Refunded = 3,        // "Refunded"
  PartiallyRefunded = 4 // "Partially Refunded"
}
```

### UI Recommendations

#### Draft Status Display
- Show badge: "⏳ Pending Publication"
- Disable "Release Funds" button
- Disable "Refund" button  
- Show tooltip: "Escrow must be published by admin first"

#### After Publish
- Show badge: "🔒 Active"
- Enable "Release Funds" button (if admin)
- Enable "Refund" button (if conditions met)

### Query Examples

```graphql
# Get all draft escrows
query {
  escrows(where: { status: DRAFT }) {
    bountyId
    depositor
    amount
  }
}

# Listen for publish events
subscription {
  escrowPublished {
    bountyId
    publishedBy
    timestamp
  }
}
```

---

## For Backend/Indexers

### Event to Index

```rust
// New event type
EscrowPublished {
    version: u32,
    bounty_id: u64,
    published_by: Address,
    timestamp: u64,
}
```

### Database Schema Updates

```sql
-- Add status column if needed
ALTER TABLE escrows 
ADD COLUMN status VARCHAR(20) DEFAULT 'DRAFT';

-- Index for filtering
CREATE INDEX idx_escrow_status ON escrows(status);
```

### API Response Format

```json
{
  "bounty_id": 123,
  "status": "DRAFT",
  "depositor": "G...",
  "amount": "1000",
  "deadline": 1234567890,
  "published_at": null  // Populated after publish
}
```

---

## Testing Commands

```bash
# Run draft state tests
cd contracts/bounty_escrow/contracts/escrow
cargo test test_draft_state --lib

# Run all tests
cargo test --lib

# Check for compilation errors
cargo check
```

---

## Common Issues & Solutions

### Issue: "InvalidState" error on release
**Cause**: Trying to release Draft escrow  
**Solution**: Call `publish()` first

### Issue: Escrow not found after lock_funds
**Cause**: Looking in wrong storage key  
**Solution**: Escrow exists but in Draft status

### Issue: Double publish attempt
**Cause**: Calling publish() on already-published escrow  
**Solution**: Check status before calling publish()

---

## Migration Checklist

### For Existing Integrations

- [ ] Update status enum to include Draft
- [ ] Handle Draft status in UI
- [ ] Add publish() function call
- [ ] Update queries to filter Draft if needed
- [ ] Listen for EscrowPublished events
- [ ] Test with Draft escrows on testnet

---

## Security Notes

### Who Can Publish?
- Admin only (same as release/refund)
- Configured during contract initialization

### Fund Safety
- ✅ Funds are secure in Draft state
- ✅ Cannot be moved until published
- ✅ Only admin can publish

### Audit Trail
- All publications emit events
- On-chain record of publish action
- Timestamp and publisher tracked

---

## Quick Troubleshooting

| Problem | Check | Solution |
|---------|-------|----------|
| Can't release funds | Is status Draft? | Call publish() |
| Publish fails | Is bounty Locked? | Already published |
| Publish fails | Does bounty exist? | Check bounty_id |
| Wrong error code | Which operation? | Check InvalidState vs FundsNotLocked |

---

## Related Documentation

- **Full Details**: `DRAFT_STATE_IMPLEMENTATION.md`
- **Overview**: `DRAFT_STATE_SUMMARY.md`
- **PR Template**: `PR_DRAFT_STATE.md`
- **Test Examples**: `test_draft_state.rs`

---

**Last Updated**: March 28, 2026  
**Version**: 1.0  
**Status**: Implemented (Bounty Escrow ✅)
