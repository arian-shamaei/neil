# Self-Improvement

Neil's autonomous debugging, learning, and self-modification system.

## Components

### failures.json
NDJSON log of every error, crash, or unexpected result. One entry per line.
Neil reviews this during idle heartbeats and either fixes issues or files
intentions for later.

Format:
```json
{"timestamp":"...","source":"component","error":"what happened","context":"where/when","severity":"low|medium|high|critical","resolution":"pending|resolved YYYY-MM-DD","notes":"..."}
```

### lessons.md
Discovered patterns, gotchas, and solutions. Loaded into essence as
persistent knowledge. This is how Neil avoids repeating mistakes.

Updated by Neil after resolving failures or discovering better approaches.
Keep entries brief and factual -- they're injected into every prompt.

### self_check.sh
Automated health check script. Verifies all Neil components are functional:
- autoPrompter binary exists and compiles
- zettel binary works
- mempalace venv is intact
- essence files present
- services handler is executable
- queue/active/history dirs exist

Returns exit 0 if healthy, exit 1 with details if something is broken.

## Self-modification rules

Neil CAN:
- Read its own source code (autoprompt.c, zettel.c, handler.sh, etc.)
- Diagnose bugs from error messages and source analysis
- Edit source files to fix bugs
- Rebuild binaries (make)
- Test changes
- Update essence/lessons.md with what was learned

Neil MUST:
- Log what was changed and why in failures.json (resolution field)
- Test after every modification
- Never modify soul.md without notifying the operator
- Never modify vault credentials
- Keep a backup before modifying source (cp file file.bak)

## Integration with heartbeat

During idle heartbeats (nothing else to do):
1. Run self_check.sh -- if anything broken, fix it
2. Review failures.json for unresolved entries
3. Pick the lowest-effort unresolved failure
4. Fix it, test, mark resolved
5. Update lessons.md with what was learned
