# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [VibeSemVer](SEMVER.md): `47.MAJOR.MINOR.TIMESTAMP`. A MAJOR
entry means read this file before upgrading. A MINOR entry means you probably
should. There is no patch level; the build timestamp is the patch.

## [Unreleased]

## [47.10.0] - 2026-10-01

Upgrading: integrations must use `user_turn` with `vfs_policy` and
`socket_policy`. Legacy session records using `sandbox_policy` or compaction
records without `replacement_history` are no longer supported.

### Changed
- Simplified built-in model guidance and limited terminal formatting instructions
  to interactive TUI sessions.
- Mouse-wheel and keyboard paging now scroll output directly in the chat pane,
  keeping the composer and other panes visible instead of opening the transcript.
  Incoming output preserves the reading position; `End` or `Esc` resumes following
  live output, while `Ctrl+T` explicitly opens the transcript viewer.
- Resuming a session already in use now shows a concise message with retry
  guidance instead of nested journal errors; diagnostic details remain in logs.

### Removed
- Legacy `user_input` submissions; callers must send `user_turn` with turn context.
- The single `sandbox_policy` field in `user_turn`; turn submissions now carry
  `vfs_policy` and `socket_policy` directly, preserving fine-grained restrictions.
- Sandbox aliases and the single `sandbox_policy` field in stored turn contexts
  and session-configured events. Both `vfs_policy` and `socket_policy` are required.
- Compaction records without `replacement_history`. Old records are rejected;
  no replay fallback or automatic migration remains.
- Deprecated raw configuration loading, automatic personality migration, PTY
  type aliases, and the unused ConPTY support shim.

## [47.9.0] - 2026-09-30

Upgrading: already-migrated credential vaults continue to work unchanged.
If credentials still use the pre-47.8 per-item Keychain store, stop ChaOS and
run `chaos config migrate-secrets` with **47.8.x before installing 47.9.0**, or
re-enter credentials in the new version. See the
[upgrade guide](README.md#upgrading-to-4790).

### Added
- Explicit `chaos hooks --yes` operator provisioning without a terminal prompt,
  including disabled legacy import followed by deliberate enabling.
- Opt-in `hook_approval_policy = "automatic"` for unattended native hook-tool
  management. Human elicitation remains the default; revision checks,
  installation-local grants, project trust, and execution sandboxing are unchanged.

### Fixed
- Agent-role application preserves the parent's hook approval policy, preventing
  trusted project roles from enabling automatic hook authorization.
- Bound the turn task's inline future size when loading database hooks, avoiding
  worker-thread stack overflows. Keep integration-test homes and working
  directories alive for lifecycle hook resolution.

### Removed
- `chaos config migrate-secrets` and the legacy settings, MCP, and provider
  Keychain import paths, including import-only vault APIs. No alias or runtime
  fallback remains. The separate `chaos config migrate` settings command is
  unchanged.

## [47.8.0] - 2026-09-29

Upgrading: stop older ChaOS processes and run `chaos config migrate-secrets`
with the new binary before restarting. The one-time import may prompt for each
legacy Keychain item. It preserves credential references and MCP approval
identities; normal operation never falls back to those legacy items. Do not run
older binaries against the upgraded vault. See
[credential vault migration](docs/database-configuration.md#credential-vault-and-macos-prompts).

### Added
- Explicit, retryable `chaos config migrate-secrets` command. Source Keychain
  items are retained for recovery; retries never overwrite live credentials or
  resurrect deleted ones. This command will be removed in **47.9.0**; migrate
  with **47.8.x** before upgrading further. See the
  [upgrade guide](README.md#upgrading-to-4790).

### Changed
- Settings, MCP credentials, named secrets, and provider auth in `keyring`/`auto`
  mode share one encrypted vault per ChaOS home, with one OS-held unlock key
  cached per process.
- Vault schema v2 uses process-shared snapshots, cross-process write locking,
  atomic owner-only writes, and deletion records that preserve logout across
  migration retries. Back up both the encrypted vault and its unlock key.

### Removed
- Plaintext fallback for provider `auto` auth. If it previously used
  `auth.json`, explicitly select `file` mode or reconnect the account into the
  vault. The existing default `file` mode and `ephemeral` mode are unchanged.
- FreeBSD onboarding's plaintext connection-URL fallback. Use `env:VARIABLE`
  when the OS credential store is unavailable.

## [47.7.1] - 2026-09-28

### Added
- Optional `case_sensitive` control for `grep_files`, using fff-search 0.11's
  explicit case modes while preserving smart-case matching by default.

### Changed
- Upgrade gix to 0.88 and use structured error classification to distinguish
  missing references, invalid inputs, and other Git failures.
- Refresh Cargo dependencies, including bonsai-bt 0.14, mcp-host 0.5.3, and
  usage-rs 6.12.

## [47.7.0] - 2026-09-28

### Added
- Database-backed lifecycle hooks for `session_start`, `before_turn`, and `stop`,
  with global and project scopes in SQLite/PostgreSQL. See
  [chaos-hooks(7)](man/chaos-hooks.7.md).
- Interactive `chaos hooks` management and optional atomic file import with
  hooks disabled by default.
- `chaos://hooks` resources and revision-checked `hooks_*` tools requiring human
  elicitation for every mutation, including disable and delete.
- Installation-local execution approvals, lifecycle refresh, sandbox enforcement,
  bounded output, and process-group cleanup on timeout or cancellation.

### Changed
- Built-in resource tools are available without external MCP servers.

## [47.6.0] - 2026-09-23

Upgrading: the model loses its git tools until skipper is installed and
registered. Install `skipper-mcp` (`just install-skipper` or the installer at
https://github.com/seuros/skipper), then run `chaos mcp add skipper -- skipper-mcp`.
See the Drivers section of `man/chaos-install.7.md`.

### Added
- `drivers/skipper` submodule: MCP driver for local git and GitHub, GitLab,
  Gitea, and Forgejo read access.

### Removed
- In-tree `git_*` tools and the `git` / `git-write` tool groups. Local git
  access for the model now comes from skipper.
- `git://branches` resource template.

[Unreleased]: https://github.com/seuros/chaos/compare/v47.10.0...HEAD
[47.10.0]: https://github.com/seuros/chaos/compare/v47.9.0...v47.10.0
[47.9.0]: https://github.com/seuros/chaos/compare/v47.8.0...v47.9.0
[47.8.0]: https://github.com/seuros/chaos/compare/v47.7.1...v47.8.0
[47.7.1]: https://github.com/seuros/chaos/compare/v47.7.0...v47.7.1
[47.7.0]: https://github.com/seuros/chaos/compare/v47.6.0...v47.7.0
[47.6.0]: https://github.com/seuros/chaos/releases/tag/v47.6.0
