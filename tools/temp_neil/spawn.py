#!/usr/bin/env python3
"""spawn_temp.py -- spin up an ephemeral Neil, run a scoped task, verify, die.

Invoked via CALL: service=spawn_temp action=run task="..." verify="..." max_sec=...

Reads from env (set by handler.sh):
  NEIL_TASK       task prompt for the temp Neil
  NEIL_VERIFY     path to verify script (optional)
  NEIL_MAX_SEC    wall-clock budget (default 300)
  NEIL_MEMORY     full | read_only | none (default read_only)
  NEIL_PERSONA    essence bundle name (default minimal)
  NEIL_HOME       parent Neil's home (for copying essence templates)

Writes report to stdout. Exit 0 on verify pass, non-zero otherwise.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def log(msg):
    print(f"[spawn_temp] {msg}", file=sys.stderr)


def main():
    task = os.environ.get("NEIL_TASK", "").strip()
    verify = os.environ.get("NEIL_VERIFY", "").strip()
    max_sec = int(os.environ.get("NEIL_MAX_SEC", "300"))
    memory = os.environ.get("NEIL_MEMORY", "read_only")
    persona = os.environ.get("NEIL_PERSONA", "minimal")
    parent_home = Path(os.environ.get("NEIL_HOME", os.path.expanduser("~/.neil")))

    if not task:
        print("ERROR: NEIL_TASK required")
        return 2

    temp_id = f"{int(time.time())}_{os.getpid()}"
    temp_home = Path(tempfile.mkdtemp(prefix=f"neil_temp_{temp_id}_"))
    log(f"temp home: {temp_home}")

    # Log spawn event to cluster activity log
    activity_log = parent_home / "state" / "cluster_activity.jsonl"
    activity_log.parent.mkdir(parents=True, exist_ok=True)
    def _activity(event: str, detail: str = ""):
        try:
            with activity_log.open("a") as f:
                f.write(json.dumps({
                    "ts": time.strftime("%Y-%m-%dT%H-%M-%S"),
                    "event": event,
                    "id": temp_home.name,
                    "parent": "main",
                    "persona": persona,
                    "memory_mode": memory,
                    "detail": detail[:200],
                }, ensure_ascii=False) + "\n")
        except Exception:
            pass
    _activity("spawn", task[:200])

    try:
        # Set up minimal essence
        essence_dst = temp_home / "essence"
        essence_dst.mkdir(parents=True)
        for fname in ["identity.md", "actions.md"]:
            src = parent_home / "essence" / fname
            if src.exists():
                shutil.copy(src, essence_dst / fname)

        # Task-specific essence layer
        (essence_dst / "task.md").write_text(
            f"""# Temporary Neil

You are an ephemeral Neil spawned for one scoped task. You have:
- no persistent memory (this home is discarded on completion)
- memory_mode={memory} (palace access restricted accordingly)
- wall-clock budget: {max_sec}s
- a verify script that determines success: {verify or '(none)'}

Your task:
{task}

When the task is complete, emit a final summary. If you can, run the
verify script via BASH and include its output in your summary. The
parent Neil will harvest your transcript and verify result. Do not
attempt to write to any filesystem location outside /tmp/ unless
explicitly necessary.
"""
        )

        # Other required dirs (so the agent's bash tool has a working environment)
        for d in ["queue", "active", "history", "state", "bin", "memory/palace/notes"]:
            (temp_home / d).mkdir(parents=True, exist_ok=True)

        # Build child env
        env = os.environ.copy()
        env["NEIL_HOME"] = str(temp_home)
        env["NEIL_PROMPT_NAME"] = f"temp_{temp_id}.md"
        env["NEIL_MEMORY_MODE"] = memory

        # PATH already set by parent autoprompt for parent's ~/.neil/bin;
        # append parent's ~/.neil/bin so temp Neil can still run neil-introspect
        env["PATH"] = f"{parent_home}/bin:{env.get('PATH', '')}"

        # Assemble system prompt from temp essence
        system_parts = []
        for f in sorted(essence_dst.iterdir()):
            system_parts.append(f.read_text())
        system_prompt = "\n\n".join(system_parts)

        # Invoke the agent
        agent_venv = parent_home / "tools/autoPrompter/agent/.venv/bin/python"
        agent_script = parent_home / "tools/autoPrompter/agent/neil_agent.py"

        log(f"invoking agent (timeout {max_sec}s)")
        t0 = time.time()
        try:
            r = subprocess.run(
                [str(agent_venv), str(agent_script),
                 "--system-prompt", system_prompt,
                 "-p", task],
                env=env, capture_output=True, text=True, timeout=max_sec,
            )
            transcript = r.stdout
            agent_exit = r.returncode
            log(f"agent done in {time.time()-t0:.1f}s exit={agent_exit}")
        except subprocess.TimeoutExpired:
            transcript = "(agent timed out)"
            agent_exit = 124
            log(f"agent TIMED OUT at {max_sec}s")

        # Run verify if provided
        verify_result = "skipped"
        verify_msg = ""
        if verify and os.path.exists(verify):
            log(f"running verify: {verify}")
            try:
                vr = subprocess.run(
                    ["bash", verify],
                    capture_output=True, text=True, timeout=60, env=env,
                )
                verify_result = "pass" if vr.returncode == 0 else "fail"
                verify_msg = (vr.stdout + vr.stderr).strip()[:800]
                log(f"verify: {verify_result}")
            except subprocess.TimeoutExpired:
                verify_result = "timeout"
                verify_msg = "verify_cmd timed out after 60s"
                log("verify timed out")
        elif verify:
            verify_result = "missing"
            verify_msg = f"verify script not found: {verify}"

        # Harvest proposed_*.json from child's state/ into parent's
        # pending_promotions.json (append-only). Failures/intentions are
        # harvested the same way -- stored in parent's state/ for later review.
        harvested = {"memories": 0, "failures": 0, "intentions": 0}
        parent_state = parent_home / "state"
        parent_state.mkdir(parents=True, exist_ok=True)
        harvest_map = [
            ("proposed_memories.json",   parent_state / "pending_promotions.json"),
            ("proposed_failures.json",   parent_state / "pending_failure_promotions.json"),
            ("proposed_intentions.json", parent_state / "pending_intent_promotions.json"),
        ]
        for src_name, dst_path in harvest_map:
            src = temp_home / "state" / src_name
            if not src.exists():
                continue
            lines = [l for l in src.read_text().splitlines() if l.strip()]
            if not lines:
                continue
            enriched = []
            harvest_ts = time.strftime("%Y-%m-%dT%H-%M-%S")
            for line in lines:
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                obj.setdefault("harvested_at", harvest_ts)
                obj.setdefault("source_instance", f"spawn_temp:{temp_id}")
                obj.setdefault("status", "pending")
                enriched.append(json.dumps(obj, ensure_ascii=False))
            if enriched:
                with dst_path.open("a") as f:
                    for line in enriched:
                        f.write(line + "\n")
                kind = src_name.replace("proposed_", "").replace(".json", "")
                harvested[kind] = len(enriched)
                log(f"harvested {len(enriched)} {kind} -> {dst_path.name}")

        # Build report
        report = [
            f"# Temp Neil Result: {temp_id}",
            f"- task: {task[:300]}",
            f"- verify: {verify or '(none)'}",
            f"- agent_exit: {agent_exit}",
            f"- verify_result: {verify_result}",
            f"- wall_clock: {time.time()-t0:.1f}s",
            f"- memory_mode: {memory}",
            f"- harvested: memories={harvested['memories']} failures={harvested['failures']} intentions={harvested['intentions']}",
            "",
        ]
        if verify_msg:
            report.append("## Verify Output")
            report.append("```")
            report.append(verify_msg)
            report.append("```")
            report.append("")
        report.append("## Transcript (truncated to 3000 chars)")
        report.append("```")
        report.append(transcript[:3000] if transcript else "(empty)")
        if transcript and len(transcript) > 3000:
            report.append(f"... ({len(transcript) - 3000} more chars)")
        report.append("```")

        print("\n".join(report))
        _activity("complete", f"verify={verify_result} harvested={sum(harvested.values())}")
        return 0 if verify_result in ("pass", "skipped") else 1

    finally:
        log(f"cleaning up {temp_home}")
        shutil.rmtree(temp_home, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
