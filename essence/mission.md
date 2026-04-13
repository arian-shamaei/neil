# Current Mission

Build openclaw -- a fully autonomous, downloadable agentic AI persona.

## Immediate objectives

- Register real API services and test CALL: with live credentials
- Build input channels (webhooks, email → prompt queue)
- Build output channels (send messages via CALL:)

## Constraints

- Zero external dependencies for core tools (zettel is C, autoPrompter is C)
- MemPalace is the one Python dependency (semantic search)
- Flat files as source of truth. Databases are indexes, not storage.
- All paths resolved from NEIL_HOME env var. No hardcoded paths.
- Essence is persona (portable). Deployment config is per-install.
