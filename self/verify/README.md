# Fulfillment Verify Scripts

Neil's INTEND: lines can carry a `verify=<path>` field. The script at
that path is the objective checker for whether the intention's work
is done. This directory holds reference templates and any scripts
Neil writes for specific intentions.

## Rules

- Exit 0 when all criteria met
- Exit non-zero with a clear message on stderr (captured into
  fulfillment_state.last_verify_msg)
- Be idempotent -- safe to run multiple times without side effects
- Complete in under 60s (enforced by verify_timeout_sec)
- Use `$NEIL_HOME` and `$NEIL_NODE_ID` from env

## Template library

### archetype_file_exists.sh
File exists and has expected structure.

### archetype_command_succeeds.sh
A command runs cleanly (build, test, script).

### archetype_state_change.sh
A specific change in the live system is observable.

### archetype_llm_judge.sh
Subjective criteria checked by an LLM-as-judge invocation.

## How Neil uses these

When filing a contracted INTEND, Neil either:

1. **Copies a template** and edits it for the specific criteria
   (most common -- produces a per-intention script like
   `verify_tilde_fix.sh`)

2. **References an archetype directly** if the criteria are generic
   (rare -- only for truly repeatable check patterns)

3. **Writes a fresh script** from scratch when no template fits
   (acceptable but prefer templates for consistency)

All verify scripts go in this directory. Neil may WRITE: new scripts
here freely; no approval needed for per-intention verify scripts.
Modifying or deleting archetype_*.sh files requires operator
approval (they're the contract library).
