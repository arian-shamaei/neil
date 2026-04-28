"""
_neil_hooks.py — Neil's runtime hook layer for third-party Python tools.

Python auto-loads this at venv startup via a sibling `.pth` file
(`_neil_hooks.pth` containing the line `import _neil_hooks`). The
`.pth` mechanism is processed by site.py *after* sitecustomize, so we
don't get shadowed by Ubuntu's system-wide
`/usr/lib/python3.12/sitecustomize.py` which sits earlier on
sys.path.

We exploit auto-load to inject Neil's instrumentation (currently:
access-flash logging into the palace's .access.jsonl) into
third-party libraries WITHOUT modifying their source. When mempalace
upgrades, we keep our hook; when this file is updated, every Python
invocation picks up the change next start.

Deployed by symlinking BOTH files into each consumer venv's
site-packages directory. Canonical source is `$NEIL_HOME/python/`.

Currently hooks:
  • mempalace.searcher.search — appends one access event per result
    to the palace's .access.jsonl with op="semantic" so the blueprint
    Graph panel renders a green pulse on each retrieved note.

Add new hooks below following the same pattern: try-import the
target module, wrap the function, fail silently on ImportError so
this file is harmless when running in a venv where the target
isn't installed.
"""
import json
import os
from datetime import datetime, timezone
from pathlib import Path


def _append_access_event(log_path: Path, note_id: str, op: str) -> None:
    """Write one JSON line to the access log. Best-effort — silent on
    any failure so instrumentation never breaks the operator's
    primary work."""
    if not note_id or note_id in ("?", ""):
        return
    try:
        ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        line = json.dumps({"id": note_id, "op": op, "ts": ts}) + "\n"
        with open(log_path, "a", encoding="utf-8") as f:
            f.write(line)
    except Exception:
        pass


def _install_mempalace_hook() -> None:
    """Wrap mempalace.searcher.search so every returned note is logged
    as op='semantic' in the palace .access.jsonl. The hook walks the
    same chromadb result data the visible search() function prints,
    via post-call invocation of the original function. We don't need
    to re-read the index — we just shadow the iteration mempalace
    already does for stdout."""
    try:
        from mempalace import searcher
    except ImportError:
        return  # not running in a venv that has mempalace installed

    if getattr(searcher, "_neil_hooked", False):
        return  # idempotent

    original_search = searcher.search

    def search_with_access_log(query, palace_path, wing=None, room=None,
                               n_results=5, **extra_kw):
        # Run the original function for its full side effects (printing
        # results, returning whatever it returns).
        result = original_search(query, palace_path, wing=wing, room=room,
                                 n_results=n_results, **extra_kw)

        # Replicate the chromadb query the original used so we can log
        # the IDs without depending on capturing stdout. This is the
        # tightest viable coupling — we're paying for one extra query
        # per search, not parsing prose.
        try:
            import chromadb
            # Match mempalace.searcher.search's invocation exactly: no
            # explicit Settings. ChromaDB caches client instances per
            # path, and a second client with mismatched settings raises
            # "different settings" — so we just use the same defaults.
            client = chromadb.PersistentClient(path=str(palace_path))
            try:
                # Mempalace's chromadb collection is named "mempalace_drawers"
                # — see searcher.py:33, :142.
                col = client.get_collection("mempalace_drawers")
            except Exception:
                return result
            where = {}
            if wing: where["wing"] = wing
            if room: where["room"] = room
            qres = col.query(
                query_texts=[query],
                n_results=n_results,
                where=where if where else None,
            )
            metas_list = qres.get("metadatas") or [[]]
            metas = metas_list[0] if metas_list else []
            log_path = Path(palace_path).parent / ".access.jsonl"
            for m in metas:
                src = m.get("source_file") if isinstance(m, dict) else None
                if not src:
                    continue
                note_id = Path(src).stem
                _append_access_event(log_path, note_id, "semantic")
        except Exception:
            # Anything goes wrong in the logging path — silent. Search
            # already returned its real result; instrumentation is gravy.
            pass

        return result

    searcher.search = search_with_access_log
    searcher._neil_hooked = True


# Auto-install on import. site.py runs this file before user code,
# so by the time anyone imports mempalace, search is already wrapped.
_install_mempalace_hook()
