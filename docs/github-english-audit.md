# GitHub English content audit

Audit date: 2026-10-08.

## Scope and policy

Reviewed existing Git-tracked files and unignored new files in the current
working tree, including hidden GitHub configuration. The initial inventory
contained 456 existing files, including 73 Markdown files. Deleted files and
ignored local build output were excluded from submission-content review.

Project code identifiers, comments, ordinary documentation, configuration text,
examples, and test descriptions use English. Dedicated localization catalogs
and their generated translations remain in their target languages, as explicitly
confirmed by the user. Unicode input used to test encoding or source positions
and necessary external paths remain valid exceptions. These exceptions are
recorded in the root rules and the VS Code module rules.

## Changes and retained data

- The VS Code README now contains the English reference only. Its repeated
  translated introductions were removed; extension localization catalogs and
  runtime translations remain intact.
- Scene specification and decision references use an English translation of the
  source conversation title while preserving its conversation identifier.
- Three existing Rust test strings use Unicode escapes without changing the
  decoded test input or its encoding and position expectations.
- Remaining non-English letters outside localization catalogs occur only in
  Unicode boundary-test input (`café`, `é`, and `α`) and the existing downstream
  synchronization document's required filename.

The scan covered all tracked and unignored text files, rather than only the
current diff. Non-ASCII letters were reviewed by context; mathematical symbols,
typographic punctuation, and Unicode test characters do not imply non-English
documentation.

## Verification and downstream impact

Passed:

- `cargo fmt --all --check`
- `cargo test --locked -p codegrid-cli -p codegrid-compiler -p codegrid-level-core`
- `node scripts/check_error_codes.js` (249 identities, 452 emitted references)
- `git diff --check`

Reviewed the downstream repository rules and synchronization requirements.
The implementation-file edits are inside test-only code and preserve decoded
values. No source acceptance, VM behavior, instruction inventory, runtime API,
scene contract, localization catalog, generated binding, or production WASM
behavior changed. Therefore no downstream consumer edits or artifact rebuilds
are required for this audit. Existing artifact provenance continues to describe
its recorded build snapshot.

This audit prepares content for submission; it does not create a commit, push
to GitHub, or declare a published release.
