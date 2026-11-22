# Core audit, 2026-11-22

Scope: Rust apply/return, journal, file writes, browser ownership, dependencies, import and installer. UI/UX excluded. No live theme Apply, session modification, deployment or product code changes in this audit. Existing working tree edits preserved.

## Findings

### F1 — medium: user.js field setter reports success without changing effective preference

field.rs userjs_span scans raw text for the first user_pref occurrence, including comments; userjs_set replaces only that occurrence. Reproduced using release CLI in a temporary HOME with `field set zen_window_scheme=1`: (a) a commented example is updated while the active value remains 0; (b) with two active definitions, the first becomes 1 while the last remains 0. Both commands exit 0. Palette browser sync shares this setter. Reads also return the first occurrence. Fix should parse active statements, use effective last definition on reads and consolidate/update active duplicate definitions on writes while preserving comments. No such fix was made during the audit.

### F2 — medium: source bundled Glass and Material You have stale export checksums

Both themes' .local/share/color-schemes/materialbright.colors differ from export.json. Files were already modified before this audit. Rust import/export/preview integration tests fail, and installer shell tests report bundled sync failures. This blocks distributing/updating those source packages correctly. Installed themes' export manifests were separately checked and no checksum mismatches were found. Do not overwrite these existing edits; resolve intended content then regenerate manifests.

## Confirmed boundaries

Browser tracked paths are user.js, chrome/userChrome.css and chrome/kolira-colors.css. Zen workspaces/sessions are personal data and must stay unmanaged. A workspace's own gradient is expected to survive theme changes; it is not a missing reset feature. The earlier suggestion to clear workspace colors was incorrect.

Checks: Rust no-fail-fast sequential suite, installer harness and clippy all-targets. Logs: /tmp/kolira-audit-rust.log, /tmp/kolira-audit-shell.log, /tmp/kolira-audit-clippy.log. Clippy completed successfully. Full results appended after completion. No real desktop round trips or AUR installs; GUI excluded.

Final Rust results: 304 passed, 3 failed. Failures in audit, first_ten and preview all report the existing Glass checksum mismatch. Shell harness exited 1, with bundled package/checksum related failures and no final aggregate line.

## Follow-up fixes requested by user

F1 implemented: lexical scan skips comments and quoted examples; last active user_pref wins on reads, all active duplicate definitions are updated on writes. Regression covers comments, duplicates, personal settings, idempotence and escaped string values containing `);`. F2 implemented: only the two materialbright SHA-256 values in Glass and Material You export.json changed, retaining the existing payload edits. UI/UX, Zen spaces and session data remain unchanged. CLI deployment uses --no-themes to avoid updating/applying host themes. Final checks recorded below.

Follow-up completed: 308 Rust tests passed, 0 failed (sequential full suite); 260 shell/installer assertions passed, 0 failed; clippy all-targets -D warnings and git diff --check passed. Updated CLI deployed with backup deploy-kolira-20261122-121201-13. Both original field-set reproductions pass with installed CLI in temporary profiles. No host theme application or GUI change. Logs: /tmp/kolira-fix12-{rust,shell,clippy,install}.log.
