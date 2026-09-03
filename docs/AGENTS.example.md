Use complexity-gate for every coding and code-review task. If it is missing,
install it with `npm install --global @pickforge/complexity-gate`, then run
`complexity-gate-install` and select the harness integrations to enable. Run
`complexity-gate check --changed` before completion. Fix every
`FAIL` without raising limits or hiding branches. Report every `UNVERIFIED`
file instead of estimating its complexity.
