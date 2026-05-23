# Cutting a release

Releases are tag-driven. Pushing a `v*` tag fires
`.github/workflows/release.yml`, which:

1. Builds binaries for `darwin-arm64`, `darwin-amd64`, and
   `linux-amd64` (musl-static).
2. Uploads each as a `.tar.gz` + sha256 to a fresh GitHub release.
3. If `RELEASE_TOKEN` is configured, generates `Formula/ccmanager.rb`
   and pushes it to `NormalUhr/homebrew-ccmanager`.

## Cutting steps

1. Make sure `main` is clean and CI is green.
2. Decide the version bump (patch / minor / major).
3. Edit `Cargo.toml` and `flake.nix` so the `version` matches the new
   tag.
4. Move the `## Unreleased` section in `CHANGELOG.md` under a new
   `## v<version> (<YYYY-MM-DD>)` heading. Leave an empty `## Unreleased`
   above it for the next cycle.
5. Commit: `release: v<version>`.
6. Tag: `git tag -a v<version> -m "v<version>"`.
7. Push: `git push origin main --follow-tags`.

The release workflow runs against the tag. Watch
`Actions → Release` in GitHub.

## crates.io (optional)

If you also want `cargo install ccmanager` to work:

```sh
cargo publish --locked
```

You need to be logged in (`cargo login <token>`) with publish rights
on the `ccmanager` crate name.

## Required secrets

- `RELEASE_TOKEN` — a GitHub Personal Access Token (classic, scope:
  `repo`) with write access to `NormalUhr/homebrew-ccmanager`. Used by
  the `update-tap` job to push a new Formula on each release. Set it
  under repo Settings → Secrets and variables → Actions.

## First-time setup checklist

- [ ] Create the empty `NormalUhr/homebrew-ccmanager` repo on GitHub
      (just an empty repo, no README).
- [ ] Create the `RELEASE_TOKEN` PAT and add as repo secret.
- [ ] On crates.io, claim the `ccmanager` crate name (`cargo publish`
      from an authenticated machine).
