# Contributing to ccmanager

Thanks for taking the time to contribute. This document is the
short-form playbook for getting set up, validating changes locally, and
sending a pull request.

## Development setup

You need a stable Rust toolchain (edition 2024, rustc ≥ 1.85) and a C
linker. On Linux you also need the X11 clipboard development headers
that the `arboard` crate links against.

```sh
git clone git@github.com:NormalUhr/ccmanager.git
cd ccmanager

# Linux only: clipboard dev libs
# Debian/Ubuntu: sudo apt-get install -y libxcb-shape0-dev libxcb-render0-dev libxcb-xfixes0-dev
# Fedora/RHEL:   sudo dnf install -y libxcb-devel

cargo build
cargo test
```

For a global `ccmanager` command that tracks your local debug builds:

```sh
just install-dev    # symlinks target/debug/ccmanager into ~/.cargo/bin/
```

For a one-shot release install:

```sh
cargo install --path . --locked
```

## Before sending a pull request

Run the same checks CI will run:

```sh
just check          # cargo fmt + clippy + build (parallel)
cargo test --all-features
```

If `just` isn't installed, the manual equivalent is:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --locked --all-features
cargo test --locked --all-features
```

## Code style

- Run `cargo fmt --all` before committing. CI rejects unformatted code.
- Clippy must be warning-free. Suppress with `#[allow(clippy::<lint>)]`
  + a one-line comment only when there's a real reason; CI runs
  `-D warnings`.
- Match the existing module structure. New top-level modules go in
  `src/` and need a `pub mod foo;` in `src/lib.rs`.
- Don't introduce code-style refactors mixed into feature commits —
  split them.
- Comments explain *why* a non-obvious choice was made. Don't restate
  what the code already says.

## Commit messages

Follow the conventional-commits-ish style already in `git log`:

```
feat(tui): add F5 refresh
fix(launcher): split cd and exec across two lines for direnv users
docs(readme): rewrite as user manual
```

Prefixes: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`. Scope
in parentheses is optional but encouraged.

The first line is a short imperative summary (≤ 70 chars). The body
explains *why* the change is needed and any non-obvious trade-offs.

## Tests

- Pure logic → unit test in a `#[cfg(test)] mod tests` block alongside
  the code.
- Logic that touches process-global state (env vars, current dir) →
  integration test in `tests/` so it gets its own test binary and
  doesn't race with library tests.
- Don't mock the filesystem; use `tempfile` to create real on-disk
  fixtures.

## Reporting bugs / requesting features

Use the issue templates in `.github/ISSUE_TEMPLATE/`. The bug template
asks for a reproduction recipe, the version (`ccmanager --version`),
and the OS / terminal you're running in — please fill these in, they
remove half the back-and-forth.

## Releases

Releases are tag-driven. Pushing a `v*` tag fires the release workflow,
which builds binaries for macOS arm64/amd64 and Linux x86_64-musl,
uploads them to a GitHub release, and (if the `RELEASE_TOKEN` secret is
set) updates the Homebrew tap. See `RELEASE.md` for the cutting
procedure.
