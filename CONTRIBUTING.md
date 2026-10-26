# Contributing to SDTally

Thanks for your interest. Bug reports, measurements from other Macs, translations and code are all welcome.

Before you start, read the [privacy and safety rules](README.md#privacy-and-safety-guarantees). They are not preferences: a change that deletes user data, adds network access, guesses a number instead of measuring it, or uses alarming language will not be merged.

For security problems, do not open an issue: see [SECURITY.md](SECURITY.md).

## Requirements

- A Mac with Apple Silicon, macOS 15 or later.
- Rust stable (the crate uses edition 2024).
- Xcode 27 for the app (older Xcode versions fail to compile it).
- Optional, for Passwordless Root: an Apple Development signing identity (a free Personal Team certificate works).

## Build

```sh
cargo build --release          # target/release/sdtally and target/release/sdc-coleta
app/montar.sh                  # builds sdtally, the app and the helper; signs; prints the .app path
app/montar.sh --abrir          # same, then opens it
```

`app/montar.sh` builds into `~/Library/Caches/sdtally-build` (override with `SDTALLY_BUILD`) and signs with the first Apple Development identity in your keychain (override with `SDTALLY_ASSINATURA="<name or SHA-1>"`). Without one it signs ad hoc: the app runs, but the root helper cannot register and Full Disk Access is lost on every rebuild.

If your checkout lives in iCloud Drive, set `CARGO_TARGET_DIR` to a folder outside it.

For quick UI work you can run the app binary from `swift build` directly, outside a bundle. Developer hooks (`SDTALLY_*` environment variables, see `app/Sources/SDTally/Foto.swift`) only work in that mode, never inside the `.app`.

## Test

Run everything CI runs before opening a pull request:

```sh
cargo test --release                                  # Rust: unit, integration, golden output, APFS fixture
cargo clippy --release --all-targets -- -D warnings   # no warnings
swift test --package-path app                         # app tests with synthetic fixtures
sh scripts/g9.sh                                      # anti-scareware check (see below)
```

Notes:

- The APFS fixture test (`tests/fixture_apfs.rs`) creates a small disk image with `hdiutil`, with a regular file, full and partial clones, a sparse file, a hard link, a compressed file and large xattrs. It needs no root. Expected values come from `st_blocks` measured in the test, never from hand-written numbers.
- `tests/golden/` holds the expected text output in Portuguese and English. If you change text output on purpose, update both.
- CI (`.github/workflows/ci.yml`) runs on macOS 15, 26 and 27. It also runs `sudo sdtally --json` on the runner and checks that the account closes (used = measured terms + unexplained, and the ranking adds up to the measured terms).

## Rules that tests enforce

### Never delete

`tests/nunca_apaga.rs` scans production code in `src/` and `app/Sources/` (including the helper) for APIs and commands that delete, trash or remove attributes (`remove_file`, `unlink`, `removeItem`, `trashItem`, `deletelocalsnapshots` and others). The only allowed occurrences are listed in the test with an exact count, and each one deletes a file SDTally created itself (old saved measurements, the helper's temporary copy of `sdtally`, and the updater's replacement of the SDTally app and its temporary download). Do not add to that list without a very good reason.

### No network

The app and the CLI never use the network. `tests/sem_rede.rs` fails if networking APIs (`URLSession`, `import Network`, `socket(`, `getaddrinfo`, `WKWebView`, `std::net`, `TcpStream` and others) appear in `src/`, `app/Sources/SDTally`, `app/Sources/SDTallyHelper` or `app/Sources/SDTallyXPC`, or if `Cargo.lock` has a networking crate. The CI `grep` job checks the same for the common APIs. The only exception is the optional updater, `app/Sources/SDTallyUpdater`, which talks only to github.com. Opening a web page in the browser (`NSWorkspace.open`) is not network access by the app.

### G9: no scareware

G9 is the project's anti-scareware charter. It is checked in three places:

- `scripts/g9.sh` (run in CI) searches the user-facing Rust sources (`src/conselho.rs`, `src/catalogo.rs`, `src/voltou.rs`, `src/report/i18n.rs`) and the app sources (`app/Sources/SDTally`) for:
  - **Banned words** from `scripts/g9-proibidas.txt`, case-sensitive, in Portuguese and English: for example "junk", "garbage", "clutter", "useless", "danger", "threat", "urgent", "infected", "virus", "speed up", "boost", "clean now". A whole line listed in `scripts/g9-excecoes.txt` is exempt; use that only for code that is not user-facing text, such as an API name.
  - In the app only: `.badge(` other than `"!"`; `.red` unless the line carries the comment `// g9: cor do Ajustes` (matching a color System Settings uses); custom colors (hex literals, `srgbRed`, `NSColor(name:)`), so only system colors are used; and any `Toggle(` in `ConselhosView.swift` (the recommendations screen).
  - It exits 0 when clean, 1 with a list of findings, 2 on bad usage. `--so-rust` checks only the Rust sources.
- `tests/g9.rs` runs the same word check over the Rust sources inside `cargo test`, and runs `scripts/g9.sh --so-rust`.
- `app/Tests/SDTallyTests/G9Tests.swift` checks that the advice levels `system_handles` and `do_not_touch` are never shown in red or orange.

The point of G9: SDTally explains, it does not frighten. Describe what something is, whose it is, what is lost if it is removed, and what the next step is. Never imply urgency or harm.

### Measure, do not estimate

Every number must come from a syscall, an API, a system log or exact arithmetic on measured numbers. If something cannot be measured, report it as `not_measured` (or `null` bytes) with a `reason`. Never use `0` to mean "unknown". JSON changes must keep `schema_version: 1` compatible (additive only) or bump it, and `docs/json-schema.md` must be updated in the same change.

## Never commit personal data

This repository is public. Measurements, fixtures, test output, screenshots and research notes must not contain:

- real home paths with a user name (use `~`, or a placeholder such as `<user>`),
- real file or folder names from your Mac,
- UUIDs, volume names, disk serials, Team IDs, account names or email addresses from your machine.

Fixtures in `tests/fixtures/` and `app/Tests/SDTallyTests/Fixtures/` are synthetic or scrubbed. If you attach a measurement to an issue, export it with "Replace Personal Names" on, or run `sdtally --redact names --stdin < measurement.json`, and still review it before posting. `.gitignore` already excludes audit outputs.

## Code style

- Match the surrounding code. The codebase is compact: short functions, comments that explain why, not what.
- Code comments and identifiers are in Portuguese. Keep the language of the file you are editing.
- Rust: `cargo clippy -D warnings` must pass. The binaries use `#![deny(unsafe_code)]`; unsafe FFI lives in `src/sys/`.
- Swift: Swift 6 strict concurrency, SwiftUI, system colors only (see G9).
- Keep changes small and focused. No drive-by refactors.

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/) in English, with a scope when it helps:

```text
feat(app): History with periods and saved measurements
fix(helper): updates no longer unregister the root helper
docs: explain the Automatic mode in the wiki
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`.

## Translations

There are two sets of translated text.

| What | Where | Format |
|---|---|---|
| App interface | `app/Sources/SDTally/Localizable.xcstrings` | String Catalog. The key is the Portuguese (Brazil) text; each of the other 35 languages is a localization. Edit with Xcode or any JSON-aware editor |
| JSON texts from the CLI (explanation cards, "why space may not come back") | `src/traducoes/<code>.json` | A flat map from the English text to the translated text. Portuguese and English live in the Rust source |

Languages follow the macOS localizations: `ar ca cs da de el es es-419 fi fr he hi hr hu id it ja ko ms nb nl pl pt-PT ro ru sk sl sv th tr uk vi zh-Hans zh-Hant`, plus `pt-BR` and `en`.

The Rust test `traducoes_validas_e_em_dia` (`src/traducao.rs`) fails if a language file is missing a translation for any current English text, or still translates a text that no longer exists. So when you change an English text in `src/`, update every `src/traducoes/*.json` in the same change.

Translations must follow G9 in every language: no alarming words, no urgency.

The CLI's own text output and messages exist only in Portuguese and English (`src/report/i18n.rs`); a missing key fails the tests.

## Pull requests

1. Fork, branch, and keep the pull request to one topic.
2. Run the full test list above.
3. Fill in the pull request template, including how you verified the change and on which macOS version.
4. For UI changes, add before and after screenshots with no personal data.

By contributing, you agree that your contribution is licensed under GPL-3.0-or-later.
