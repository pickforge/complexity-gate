# Install PickCheck

You are an AI coding agent. Install PickCheck for the user. Perform the
installation and verification instead of only describing the commands.

PickCheck was formerly named complexity-gate. Since 0.3.0 the npm package,
binary, installer, hook commands, config file, and state directory all use the
`pickcheck` name. If an older install exists, first follow the
[rename steps in the README](README.md#renamed-from-complexity-gate) to remove
its hooks, plugins, and package, then continue below; the installer never
removes old entries.

## Choose integrations

Supported harnesses are Claude Code, Codex, Pi, OMP, Grok, Cursor, and
OpenCode.

If the user named the harnesses, use that selection. Otherwise, detect the
harnesses installed on the machine and ask one short question listing them.
Allow the user to choose a comma-separated list, `all`, or `none`. `none` means
install only the command-line tool.

Do not enable an integration the user declined. Preserve existing settings and
hooks.

## Install

Check that Node.js 22 or newer and npm are available. Then run:

```sh
npm install --global @pickforge/pickcheck
```

Do not use `sudo` to work around npm permission errors. Use the user's existing
Node version manager or explain the permission problem.

If Node.js 22 cannot be used, install the binary from the verified archive on
[GitHub Releases](https://github.com/pickforge/pickcheck/releases) or
build it with Cargo. Explain that Pi, OMP, and OpenCode plugin installation
still requires their package tooling.

## Configure hooks and plugins

Run the installer non-interactively with the user's selection. Replace the
example list with the chosen harnesses:

```sh
pickcheck-install --harness claude,codex
```

Use `pickcheck-install --all` only when the user selected all harnesses.
The installer merges JSON hooks without replacing unrelated configuration. It
uses the native Pi, OMP, and OpenCode package commands. Grok reuses compatible
Claude Code or Cursor hooks when present so the gate does not run twice.

## Add agent instructions

If the user wants PickCheck used for every coding and review task, add the
following text to the project or global agent instruction file they choose.
Preserve the existing file. If they did not choose project or global scope, ask
before editing it.

```md
Use PickCheck (`pickcheck`) for every coding and code-review task. Run
`pickcheck check --changed` before completion. Fix every `FAIL` without
raising limits or hiding branches. Report every `UNVERIFIED` file instead of
estimating its complexity.
```

## Verify

Run these commands from a Git repository:

```sh
pickcheck --version
pickcheck doctor
pickcheck check --changed
```

Run the final command from inside the repository. It intentionally fails instead
of scanning the current directory when no Git `HEAD` is available. If it reports
violations, run the printed scoped `--verbose` command for one file at a time.

Confirm the selected harness configuration contains `pickcheck`, without
printing unrelated configuration or secrets. For Pi, OMP, and OpenCode, use
their plugin-list or resolved-config command when available.

Report the installed version, enabled harnesses, instruction-file scope, and
verification results. If anything failed, report the exact failed step and
leave existing configuration intact.
