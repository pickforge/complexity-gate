# Hook mapping

Verified against the live documentation on **2026-09-03**:

- [Claude Code hooks](https://docs.anthropic.com/en/docs/claude-code/hooks)
- [Codex hooks](https://developers.openai.com/codex/hooks)
- [Cursor hooks](https://cursor.com/docs/hooks)
- [Grok hooks](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/10-hooks.md)

| Harness | Edited-file event | Completion event | Violation result |
|---|---|---|---|
| Claude Code | `PostToolUse` for `Edit`, `Write`, `MultiEdit` | `Stop` | `decision: "block"` JSON |
| Codex | `PostToolUse` for `apply_patch`, `Edit`, `Write` | `Stop` | `decision: "block"` JSON |
| Cursor | `afterFileEdit` with top-level `file_path` | `stop` with `status: "completed"` | edit annotation on stderr; stop `followup_message` JSON |
| Grok | `post_tool_use` for edit tools | `stop` | edit annotation on stderr; stop exit 2 with reason on stderr |

## Limitations

Codex exposes an `apply_patch` command string, not a stable edited-file field.
For that event the gate checks all functions touched in the Git diff. This is
less precise if unrelated working-tree edits already exist. Neither post-tool
hook can undo an edit that already happened.

Cursor and Grok post-edit hooks are passive, so immediate findings appear in
their hook output rather than becoming agent instructions. Their completion
hooks still force a correction pass.

The stop loop guard is shared across harnesses. Counters are keyed by the
sanitized, length-capped session or conversation ID under `~/.pickforge/complexity-gate/`, or
`COMPLEXITY_GATE_HOME` when set. The hook blocks the first `hook.max_blocks`
consecutive failing Stop events. Later failing Stop events report `UNRESOLVED`
on stderr and do not reset the counter; only a clean Stop resets it. A missing
ID uses the `unkeyed` counter. Working-directory fallbacks use each harness's
workspace field or environment variable before the process directory.

Claude Code's `stop_hook_active` input field is intentionally ignored. The
local counter is the loop guard for every harness, regardless of whether a
Stop event was triggered by another Stop hook continuation.
