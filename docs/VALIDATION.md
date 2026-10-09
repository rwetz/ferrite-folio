# Validation, 2026-10-09

- `cargo check --offline`: passed, using the shared cached target directory.
- `cargo clippy --offline -- -D warnings`: passed.
- Isolated service-module tests against cached dependencies: 2 catalogue/Unicode paging tests passed.
- Live service smoke: Searched Gutendex and downloaded a Gutenberg text edition.
- Full native `cargo test`/build and GUI walkthrough: not performed in this pass.

The checked-in `--smoke` entry point provides a repeatable service check once the
binary is built. Parser tests also run under the normal `cargo test` command.

Desktop layout, keyboard navigation, scrolling, and dark/light appearance still
need a native walkthrough. These are development scaffolds, not release-ready apps.
