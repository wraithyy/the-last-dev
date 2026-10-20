# The Last Dev

Rust/Bevy browser RPG. Commands, layout and debug hooks are in [README.md](README.md); design docs in `docs/`.
The landing page (Astro) lives in `site/`.

## Changelog and versions

Every player-visible change gets a line in [CHANGELOG.md](CHANGELOG.md) in the same commit that makes it.

- Add it under `## [Unreleased]`, in the right group: `### Added`, `### Changed`, `### Fixed` or
  `### Removed` (create the group if it is missing; keep that order).
- Write for players, not developers: what they can now do or what stopped breaking, one sentence,
  past the commit subject's shorthand. "Loot no longer crashes WebGL2" beats "fix: convert icon PNGs".
- Skip changes players cannot notice: refactors, tests, CI, docs, tooling, debug hooks.
- Never edit a released section; a correction to a shipped version goes into the next one.

The version lives only in `[workspace.package] version` in `Cargo.toml`; crates, the Esc menu and the
site read it from there. Never set a version anywhere else.

Release only when the user asks for one. While the game is 0.x: content (zones, classes, systems,
raids) bumps the minor version, fixes alone bump the patch. Steps are in README.md under "Releasing";
the `vX.Y.Z` tag push creates the GitHub Release from the changelog section and fails if the tag and
`Cargo.toml` disagree.
