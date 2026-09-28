+++
title = "chaos-hooks(7)"
summary = "Database-backed lifecycle hooks, human approval, and execution."
+++

# chaos-hooks(7)

## NAME

chaos-hooks - manage database-backed lifecycle hooks

## DESCRIPTION

Hooks run shell commands on `session_start`, `before_turn`, or `stop`. Definitions
live in the existing SQLite/PostgreSQL database, independently of user settings.
Neither global, system, nor project `hooks.json` files are loaded at runtime.

## MANAGEMENT

```sh
chaos hooks list
chaos hooks add context --event before-turn --command 'git status --short'
chaos hooks show context
chaos hooks enable context
chaos hooks disable context
chaos hooks update context '{"event":"before_turn","command":"git diff --stat"}'
chaos hooks remove context
```

Add `--project` to register for the current canonical project root. Otherwise the
hook is global. IDs are unique in the database. Add defaults to disabled;
`--enabled` explicitly requests recurring execution. Update replaces the whole
definition and preserves enabled state. Every CLI mutation requires interactive
confirmation; inspection never executes commands.

`chaos hooks import /path/to/hooks.json --prefix legacy [--project]` is an
explicit one-time import. It validates the entire document, rejects unsupported
handlers, and imports atomically as disabled hooks. Existing IDs cause rollback,
so retries cannot duplicate hooks. Original files are not deleted. Enable each
imported hook separately.

## MODEL ACCESS

- `chaos://hooks`: visible global/current-project definitions, revisions,
  installation approval state, and reasons a hook is inactive.
- `chaos://hooks/{id}`: one visible definition.
- `hooks_create`, `hooks_update`, `hooks_set_enabled`, `hooks_delete`: propose a
  change, then wait for human form elicitation. Even disable/delete need approval.
- `hooks_preview`: validate a proposed action without writing or executing.

Read resources again after changes; built-in subscriptions return snapshots.
Management tools are in the `session` capability group and mutations obey the
active mode's mutation capability.

The human sees the exact old/new definition, event, project scope, current
working directory, timeout, and whether recurring execution is being authorized.
The client must return `accept` with `approve: true`. Decline, cancel, timeout,
disconnect, headless mode, and unavailable elicitation never change a hook.
There is no model-callable approval endpoint or `approved` input. Updates require
the current `expected_revision`; a concurrent change invalidates the proposal.
Creating with the same ID never creates a duplicate.

Approvals are installation-local and bound to the exact stored revision. Editing
a hook invalidates previous approvals on all installations; only the approving
installation receives the new grant. Generic config import does not activate
hooks. `chaos approvals revoke` also revokes hook authorization.

## EXECUTION

Project hooks require current database project trust. The kernel reloads hooks
at lifecycle boundaries; preview and execution share a snapshot. Changes do not
interrupt a dispatch already running. Storage/validation failures stop the turn
rather than silently skipping hooks. No filesystem fallback exists.

Commands run in the session's working directory with its filtered environment,
current filesystem/network sandbox, and execution-policy deny checks. Hook
approval does not grant sandbox escalation. Timeout covers stdin, process exit,
and output draining (default 30 seconds, maximum 600); each output stream is
limited to 128 KiB. Hook process groups are terminated on cancellation/timeout.
Existing event input/output JSON semantics are unchanged. Results are ordered
global-before-project, then by `order` and stable ID; matching commands can run
concurrently. Event origins use `chaos://hooks/{id}`.

Do not embed credentials in command strings; those strings are visible through
resources and approvals. Refer to environment variables instead. Approval gates
protect managed APIs, not an attacker with root access or database credentials.

## SEE ALSO

- [chaos-mcp.7](./chaos-mcp.7.md)
- [chaos-storage.7](./chaos-storage.7.md)
