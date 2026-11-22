# Zen: legacy invalid user.js assignments

Implemented 2026-11-22. Validation results recorded in the task response.

Native user.js contained eight INI-style appearance assignments before valid Paper preferences. The pink value matches sakura-dream; the original writer is not established. prefs_merge preserved unrecognised lines, and the baseline journal preserved the malformed file. Browser palette sync also preserved them.

Both merge and palette accent generation now remove only known browser appearance assignments with JSON literal values. Unknown lines and personal preferences remain intact. Regression coverage checks cleanup and idempotence. Live native user.js repaired with backup under ~/.config/kolira/backups/zen-userjs-repair-20261122-114327. Browser restart / visual verification remains user-side; no browser was stopped.

Validation: 142 unit tests pass sequentially; integration suites actions (7) and apply_protection (16) pass. Full integration run stops at audit: existing modified glass materialbright.colors fails the bundled checksum. These pre-existing theme edits were preserved. Parallel unit run had an unrelated fish/OMP test failure, absent in sequential run.
