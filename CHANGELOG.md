# Changelog

## [1.5.0] — Changes since v1.4.4

### Multi-Node Account System

- Added support for linking multiple Meshtastic radios to a single BBS account, sharing posts, DMs, board read positions, and bios across all nodes.
- Introduced a full invitation system for joining multi-node accounts with commands: `invite send`, `accept`, `deny`, `pending`, `block`, `unblock`, `leave`, and `remove`.
- Invitations use pronounceable passwords (e.g. `bozwenkatfig`) for easy out-of-band sharing. Only one outbound invitation is allowed at a time, with a 24-hour expiry and 1-hour cooldown after denial.
- `invite accept` supports an optional `migrate` flag to reassign old posts and DMs to the new account; without it, the old account becomes a ghost.
- `invite status` (via the `?` command) shows which nodes are on your account and whether invitations are blocked or allowed.
- Account IDs are now displayed alongside usernames in user listings.

### Configuration

- Menu configuration has been split out of `config.toml` into a separate `menus.toml` file.
- The `menus` field is now optional in `config.toml`. A warning is displayed when no menus are configured.
- Cleaned up and improved the default menu layout, including retaining the Sysop menu.

### Bug Fixes

- Fixed ping command letter casing.
- Reject duplicate posts to prevent resubmission when radio connections are lost.

### Improvements

- Weather observation times are now shown in the local timezone instead of UTC.
- More readable "Replying to..." info messages.
- Added `frozenbbs --whoami` for long version output showing the current node identity.
- The BBS now checks for pending database migrations at startup.

### Performance & Build

- Switched from `vergen-git2` to `vergen-gix` for faster compilation.
- Disabled unused config format backends to reduce compile time.
- Removed unused diesel `time` feature.
- Replaced `once_cell` crate with `std::sync::LazyLock` from the standard library.
- Skip unnecessary rebuild step before installing.
- Track `Cargo.lock` in version control for reproducible builds.

### Dependencies

- Upgraded `rand` to 0.10 and `toml` to 1.0.
- General dependency updates.
