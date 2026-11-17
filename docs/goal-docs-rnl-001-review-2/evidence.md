# Verified Evidence — rnl-001-review-2

| ID | Fact | Source (`file:line` / command) | Method | Confidence | Round |
|---|---|---|---|---|---|
| EVD-001 | Bit-only budget was ignored before the fix | docs/goal-docs-rnl-001-review-2/round-1/bit-budget.md §2 | Pre-fix test output: Ok(2/1) vs required BitBudgetExceeded{limit:1,observed:3} | High | 1 | OBS-001 |
| EVD-002 | Normalized value checks both digit and bit limits | lovelace-rational/src/lib.rs:372/385 | Zero and post-GCD paths call limits.check; focused tests pass | High | 1 | OBS-001 |
| EVD-003 | Budget combinations are covered | lovelace-rational/tests/properties.rs:584-742 | bit-only, digit-only, combined, zero and reduced tests pass; 16 property tests total | High | 1 | OBS-001 |
| EVD-004 | Partial duplicate kernel was not detected before the fix | docs/goal-docs-rnl-001-review-2/round-1/partial-duplicate.md §1 | Pre-fix mutation run: partial-duplicate-kernel got exit 0 | High | 1 | OBS-002 |
| EVD-005 | Partial duplicate kernel is rejected after the fix | python tools/test-architecture-gates.py | 21/21 scenarios pass; partial-duplicate-kernel exits 1 with duplicate-numeric-representation | High | 1 | OBS-002 |
| EVD-006 | Real workspace and valid fixture stay clean | python tools/check-architecture.py; valid fixture command | 14 crates/332 files/0 violations; valid fixture 7/7/0 | High | 1 | OBS-002 |
| EVD-007 | Final local suite is 6280/0/0 | cargo test --workspace --no-fail-fast; per-crate loop | 45 targets, 6280 passed, 0 failed, 0 ignored; rational 34 | High | 1 | OBS-003 |
| EVD-008 | Second-review head passes all seven PR checks | gh pr checks 9; run 44140509356 | All seven checks pass at 93fbf38; mergeStateStatus CLEAN | High | 2 | OBS-003 |
