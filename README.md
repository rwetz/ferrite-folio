# Folio

A Project Gutenberg reader using Gutendex for discovery.

A working development scaffold, not a published release.

## Run

Requires Rust 1.92 or later and the normal GPUI native toolchain. Ferrite Design
is pinned to a published GitHub commit and fetched automatically by Cargo.

```powershell
git clone https://github.com/rwetz/ferrite-folio.git
cd ferrite-folio
cargo run
```

No API key is required.

## Current slice

- Search English public-domain titles and authors.
- Download supported UTF-8 plain-text editions from Gutenberg.
- Read the full source in Unicode-safe pages, with previous/next controls.
- Network operations run off the UI thread, with time and download limits.

The title-bar menu and Ctrl+Shift+P expose actions and scheme/appearance changes.
`FERRITE_SCHEME`, `FERRITE_APPEARANCE`, and `FERRITE_FPS` supply launch defaults.
These scaffolds do not yet persist their own theme or use the planned shared settings store.

## Next

Local library/cache, bookmarks and reading-position persistence, chapter navigation,
font/line-spacing controls, and catalogue pagination. Search currently shows the first API page.
Book availability is filtered using catalogue copyright metadata; check the original book's terms for your jurisdiction.

## API reference

[Official integration documentation](https://gutendex.com/)

## Validation

```powershell
cargo check
cargo clippy -- -D warnings
cargo test
cargo run -- --smoke           # verify the real service without opening a window
```

Compilation and service tests do not establish native window, scrolling, or device quality.
