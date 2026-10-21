## What and why

<!-- What does this change do, and why? Link the issue if there is one. -->

## How I verified it

<!-- Commands you ran and on which macOS version. For UI changes, add before and after screenshots with no personal data. -->

- macOS version:
- [ ] `cargo test --release`
- [ ] `cargo clippy --release --all-targets -- -D warnings`
- [ ] `swift test --package-path app` (if the app changed)
- [ ] `sh scripts/g9.sh`

## Checklist

- [ ] Read-only: nothing deletes, moves to Trash or modifies user data (`tests/nunca_apaga.rs` passes).
- [ ] No network access added.
- [ ] Every new number is measured and has a source; unmeasured values are `null` with a reason, never `0`.
- [ ] User-facing text follows G9 (no alarming words, no urgency).
- [ ] JSON changes are additive, or `schema_version` is raised, and `docs/json-schema.md` is updated.
- [ ] Changed English texts in `src/` are updated in every `src/traducoes/*.json`; new app strings are in `Localizable.xcstrings`.
- [ ] No personal data: no real user paths, file names, UUIDs, volume names or account names in code, fixtures, tests or screenshots.
- [ ] Commit messages follow Conventional Commits.
