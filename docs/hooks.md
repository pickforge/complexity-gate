# Hook mapping

Verified by fetching the live documentation on **2026-08-27**:

- [Claude Code hooks](https://docs.anthropic.com/en/docs/claude-code/hooks)
- [Codex hooks](https://developers.openai.com/codex/hooks)

Both inputs use `hook_event_name`, `session_id`, `cwd`, `tool_name`, and
`tool_input`. Gate findings always exit 0 with JSON; malformed input and runtime
failures exit non-zero with one stderr line.

| Gate action | Claude Code | Codex |
|---|---|---|
| Check an edited file | `PostToolUse`, tools `Edit`, `Write`, `MultiEdit`; absolute `tool_input.file_path` | `PostToolUse`, canonical tool `apply_patch` (aliases `Edit`/`Write`) |
| Check before completion | `Stop` | `Stop` |
| Return feedback | `{"decision":"block","reason":"…"}` adds feedback after the tool | The same object replaces the completed tool result with feedback |
| Continue after stop | `decision: "block"` prevents stopping | `decision: "block"` creates a continuation prompt from `reason` |

## Limitations

Codex exposes an `apply_patch` command string, not a stable edited-file field.
For that event the gate checks all functions touched in the Git diff. This is
less precise if unrelated working-tree edits already exist. Neither post-tool
hook can undo an edit that already happened.

The stop loop guard is shared across harnesses. Counters are keyed by a
sanitized, length-capped `session_id` under `~/.pickforge/complexity-gate/`, or
`COMPLEXITY_GATE_HOME` when set. The hook blocks the first `hook.max_blocks`
consecutive failing Stop events. Later failing Stop events report `UNRESOLVED`
on stderr and do not reset the counter; only a clean Stop resets it. A missing
`session_id` uses the `unkeyed` counter, and a missing `cwd` uses the process
working directory.

Claude Code's `stop_hook_active` input field is intentionally ignored. The
local counter is the loop guard for both harnesses, regardless of whether a
Stop event was triggered by another Stop hook continuation.
