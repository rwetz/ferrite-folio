# Folio

A Project Gutenberg reader using Gutendex for discovery.

An early development release. [Windows downloads](https://github.com/rwetz/ferrite-folio/releases/latest) are also installable through Lodestone.

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

## Automatic releases

Every push to `main` runs the release workflow. It chooses the next available
patch version (or honors a higher version set in `Cargo.toml`), creates a release
commit with matching `Cargo.toml` and `Cargo.lock` versions, and tags that commit.
Release commits stay off `main`, so the workflow cannot create a push loop.
Builds and tests must succeed before publication. Rerunning a workflow reuses
its tag. Manual `vMAJOR.MINOR.PATCH` tags still work when they match the package
version. Push feature branches normally; they do not publish releases.

## Catalogue availability

If Gutendex is unavailable, Folio searches Gutenberg's official English text
catalogue. The initial catalogue download is cached locally for later searches.
Book downloads are bounded, and service errors remain visible in the window.

Each Windows executable includes its own pixel icon. `assets/logo.svg` also
supplies the matching catalogue icon in Lodestone.
