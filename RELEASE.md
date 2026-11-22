# Releasing

A release is a version-bump commit on `main` plus a signed `vX.Y.Z` tag.
Pushing the tag is what publishes: `.github/workflows/release.yml` builds
`ttcli` for four targets and creates the GitHub Release with the archives
attached. Nothing is published to crates.io or npm.

## Steps

Replace `X.Y.Z` throughout. Pick the number from the conventional commits
since the last tag (`fix:` → patch, `feat:` → minor while on `0.x`).

1. **Start clean and green.**

   ```bash
   git switch main && git pull
   git status                                   # must be clean
   cd site && yarn install && yarn build && cd ..
   just gate
   ```

2. **Bump the version in all three files**, by hand:

   - `Cargo.toml`
   - `cli/Cargo.toml`
   - `package.json`

   `site/package.json` stays at `0.0.0`; it is not part of the release.

3. **Refresh `Cargo.lock`.**

   ```bash
   cargo build --release
   ```

4. **Generate the changelog**, then read it.

   ```bash
   just changelog
   git diff CHANGELOG.md
   ```

   This must come after step 2 and before the tag exists — see
   [Ordering](#ordering-matters).

5. **Commit, tag, push.**

   ```bash
   git add Cargo.toml cli/Cargo.toml Cargo.lock package.json CHANGELOG.md
   git commit -m 'chore: vX.Y.Z release'
   git tag -s vX.Y.Z -m 'vX.Y.Z'
   git push --follow-tags origin main
   ```

6. **Check it published.**

   ```bash
   gh run watch          # the "Release Build" run for the tag
   gh release view vX.Y.Z
   ```

   The release should carry four archives: `intel-mac`, `silicon-mac`,
   `linux-x64` (`.tar.gz`) and `windows-x64` (`.zip`).

## Ordering matters

`just changelog` runs `standard-version` with the bump, commit and tag steps
all skipped, so it only writes `CHANGELOG.md`. It takes the new version from
`package.json` and the range from the latest `v*` tag. Two consequences:

- **`package.json` not bumped yet** → it writes nothing, because that
  version's tag already exists.
- **Tag already created** → the range is empty and it writes nothing.

Both happened during the v0.9.0 release and cost a deleted tag, a rebase and
a force-push. If the changelog comes out empty, one of these is why.

## Review the generated changelog

The generator copies a commit's `BREAKING CHANGE:` footer verbatim, along
with everything after it in the commit body. A long body ends up pasted into
the `⚠ BREAKING CHANGES` section, trailers included (the v0.9.0 entry is an
example). Trim that section by hand before committing.

`refactor:` and `test:` commits are hidden; the sections and link formats are
set in `.versionrc.json`.

## If a release goes wrong

- **Tag not pushed yet:** `git tag -d vX.Y.Z`, fix, re-tag.
- **Tag pushed, build failed for an unrelated reason:** re-run the workflow
  from the Actions tab (`workflow_dispatch`, with the tag as input) rather
  than moving the tag.
- **Tag pushed and wrong:** prefer a new patch release over moving a
  published tag.

## Not part of the process

Two leftovers from October 2025 look like release entry points and are not:

- **`npm run release`** (`standard-version -s`) was used for v0.5.1 and
  v0.5.2. It bumps only `package.json`, so the Cargo versions drift, and it
  commits as `chore(release): X.Y.Z`. Superseded by the steps above from
  v0.6.0 on.
- **`release.toml`** configures `cargo-release` with a `git-cliff` changelog
  hook. No tagged release has gone through it — its tag message and
  changelog format match none of the existing tags or `CHANGELOG.md`.
