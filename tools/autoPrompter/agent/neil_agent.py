#!/usr/bin/env python3
"""
neil_agent.py - Tool-enforced agent replacing `claude --print` for Neil.

Replaces text-parsed action lines (BASH:, READ:, WRITE:, CALL:) with
structured tool use via the Claude Agent SDK. Tool calls are enforced
by the API -- Claude cannot answer from context and skip execution.

Declarative actions (MEMORY:, HEARTBEAT:, INTEND:, DONE:, FAIL:, NOTIFY:,
PROMPT:) remain as text lines in the final output -- the C daemon
(autoprompt.c) still post-processes those for logging.

Usage (mirrors claude --print):
    neil_agent.py --system-prompt <path-or-text> -p <prompt-or-path>

Stdin prompt is also supported: neil_agent.py --system-prompt X < prompt
"""

import anyio
import asyncio
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

from claude_agent_sdk import (
    ClaudeAgentOptions,
    AssistantMessage,
    ResultMessage,
    SystemMessage,
    TextBlock,
    ToolUseBlock,
    ToolResultBlock,
    create_sdk_mcp_server,
    query,
    tool,
)


# ─── Tool implementations ─────────────────────────────────────────────

@tool("read_file", "Read a file from the filesystem. Returns content (max 50KB).", {"path": str})
async def read_file(args: dict) -> dict:
    path = os.path.expanduser(args["path"])
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            content = f.read(50_000)
        return {"content": [{"type": "text", "text": content}]}
    except Exception as e:
        return {"content": [{"type": "text", "text": f"ERROR: {e}"}], "is_error": True}


@tool("write_file", "Write content to a file (creates or overwrites).",
      {"path": str, "content": str})
async def write_file(args: dict) -> dict:
    path = os.path.expanduser(args["path"])
    content = args["content"]
    try:
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            f.write(content)
        return {"content": [{"type": "text", "text": f"wrote {len(content)} bytes to {path}"}]}
    except Exception as e:
        return {"content": [{"type": "text", "text": f"ERROR: {e}"}], "is_error": True}


@tool("bash", "Run a shell command. Returns stdout+stderr. 60s timeout.", {"command": str})
async def bash(args: dict) -> dict:
    cmd = args["command"]
    try:
        proc = await asyncio.create_subprocess_shell(
            cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
        )
        try:
            stdout, _ = await asyncio.wait_for(proc.communicate(), timeout=60)
            output = stdout.decode("utf-8", errors="replace")
            if len(output) > 20_000:
                output = output[:20_000] + "\n... (truncated)"
            return {"content": [{"type": "text",
                    "text": f"$ {cmd}\n{output}\n[exit {proc.returncode}]"}]}
        except asyncio.TimeoutError:
            proc.kill()
            return {"content": [{"type": "text",
                    "text": f"$ {cmd}\nERROR: timeout (60s)"}], "is_error": True}
    except Exception as e:
        return {"content": [{"type": "text", "text": f"ERROR: {e}"}], "is_error": True}


@tool("call_service", "Call a registered service via handler.sh. Use this for API calls.",
      {"service": str, "action": str, "params": str})
async def call_service(args: dict) -> dict:
    neil_home = os.environ.get("NEIL_HOME", os.path.expanduser("~/.neil"))
    service = args["service"]
    action = args["action"]
    params = args.get("params", "")

    # Validate service exists
    reg = Path(neil_home) / "services/registry" / f"{service}.md"
    if not reg.exists():
        return {"content": [{"type": "text",
                "text": f"ERROR: service={service} not registered"}], "is_error": True}

    # Read credential
    vault = Path(neil_home) / "services/vault" / f"{service}.key"
    if not vault.exists():
        return {"content": [{"type": "text",
                "text": f"ERROR: no credential in vault for {service}"}], "is_error": True}
    cred = vault.read_text().strip()

    handler = Path(neil_home) / "services/handler.sh"
    env = os.environ.copy()
    env.update({
        "NEIL_SERVICE": service,
        "NEIL_ACTION": action,
        "NEIL_CRED": cred,
        "NEIL_PARAMS": params,
    })
    try:
        proc = await asyncio.create_subprocess_exec(
            "bash", str(handler),
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT,
            env=env,
        )
        stdout, _ = await asyncio.wait_for(proc.communicate(), timeout=60)
        return {"content": [{"type": "text", "text": stdout.decode("utf-8", errors="replace")}]}
    except Exception as e:
        return {"content": [{"type": "text", "text": f"ERROR: {e}"}], "is_error": True}


# ─── Stream writer ─────────────────────────────────────────────────────

class Streamer:
    """Writes to .neil_stream (for TUI) AND accumulates a transcript (for result file).

    The transcript is returned via get_transcript() and printed to stdout so the
    C daemon captures it into the result file. Both outputs use the same format
    so the command log parser works consistently.
    """

    def __init__(self, neil_home: Path, prompt_name: str):
        self.path = neil_home / ".neil_stream"
        self.fd = open(self.path, "w", encoding="utf-8")
        self.fd.write(f'{{"status":"running","prompt":"{prompt_name}"}}\n')
        self.fd.flush()
        self._tool_names: dict[str, str] = {}
        self._transcript: list[str] = []

    def _write(self, s: str) -> None:
        self.fd.write(s)
        self.fd.flush()
        self._transcript.append(s)

    def text(self, s: str) -> None:
        self._write(s)

    def tool_call(self, use_id: str, name: str, args: dict) -> None:
        self._tool_names[use_id] = name
        short_name = name.replace("mcp__neil__", "")
        if short_name == "read_file":
            self._write(f"\nREAD: {args.get('path', '')}\n")
        elif short_name == "write_file":
            content = args.get("content", "")
            self._write(f"\nWRITE: path={args.get('path', '')} ({len(content)} bytes)\n")
        elif short_name == "bash":
            self._write(f"\n```bash\n$ {args.get('command', '')}\n")
        elif short_name == "call_service":
            detail = f"service={args.get('service','')} action={args.get('action','')} {args.get('params','')}"
            self._write(f"\nCALL: {detail}\n")

    def tool_result(self, use_id: str, content: str) -> None:
        name = self._tool_names.get(use_id, "")
        short_name = name.replace("mcp__neil__", "")
        if short_name == "bash":
            body = content
            if body.startswith("$ "):
                nl = body.find("\n")
                if nl != -1:
                    body = body[nl + 1:]
            if body.rstrip().endswith("]"):
                lines = body.rstrip().split("\n")
                if lines and lines[-1].startswith("[exit"):
                    body = "\n".join(lines[:-1]) + "\n"
            self._write(body if body.endswith("\n") else body + "\n")
            self._write("```\n")
        elif short_name == "read_file":
            preview = content[:400]
            self._write(f"{preview}\n")
            if len(content) > 400:
                self._write(f"... ({len(content) - 400} more bytes)\n")
        elif short_name == "write_file":
            self._write(f"{content}\n")
        elif short_name == "call_service":
            preview = content[:400]
            self._write(f"{preview}\n")

    def get_transcript(self) -> str:
        return "".join(self._transcript)

    def close(self, exit_code: int) -> None:
        self.fd.write(f'\n{{"status":"done","exit_code":{exit_code}}}\n')
        self.fd.close()


# ─── Main driver ─────────────────────────────────────────────────────

def parse_args(argv: list[str]) -> dict:
    """Parse claude-style args: --system-prompt X -p Y
    Also accepts --system-prompt-file <path> to read system prompt from
    a file (avoids Linux MAX_ARG_STRLEN limit of 128KB per argv element
    when essence + persona overlay exceeds that)."""
    out = {"system_prompt": None, "system_prompt_file": None, "prompt": None}
    i = 0
    while i < len(argv):
        a = argv[i]
        if a in ("--system-prompt", "--system"):
            out["system_prompt"] = argv[i + 1]; i += 2
        elif a == "--system-prompt-file":
            out["system_prompt_file"] = argv[i + 1]; i += 2
        elif a == "-p":
            out["prompt"] = argv[i + 1]; i += 2
        elif a in ("--print", "--dangerously-skip-permissions"):
            i += 1  # legacy claude flags, no-op
        elif a.startswith("--output-format"):
            i += 2
        else:
            i += 1
    return out


async def run_agent(prompt: str, system_prompt: str, streamer: Streamer) -> str:
    """Drive the agent loop, stream output, return final text."""

    tools = [read_file, write_file, bash, call_service]
    mcp_server = create_sdk_mcp_server(name="neil", version="1.0.0", tools=tools)

    # MCP tool names get prefixed: mcp__<server>__<tool>
    allowed = [f"mcp__neil__{t.name}" for t in tools]

    options = ClaudeAgentOptions(
        system_prompt=system_prompt,
        mcp_servers={"neil": mcp_server},
        allowed_tools=allowed,
        tools=[],  # disable built-in Claude Code tools; only mcp__neil__* available
        max_turns=int(os.environ.get("NEIL_MAX_TURNS", "10")),
        permission_mode="bypassPermissions",
    )

    final_text_parts: list[str] = []

    async for msg in query(prompt=prompt, options=options):
        if isinstance(msg, AssistantMessage):
            for block in msg.content:
                if isinstance(block, TextBlock):
                    streamer.text(block.text)
                    final_text_parts.append(block.text)
                elif isinstance(block, ToolUseBlock):
                    streamer.tool_call(block.id, block.name, block.input or {})
        elif hasattr(msg, "content"):
            # UserMessage with tool results
            content = getattr(msg, "content", None)
            if isinstance(content, list):
                for block in content:
                    if isinstance(block, ToolResultBlock):
                        result_text = ""
                        if isinstance(block.content, list):
                            for c in block.content:
                                if isinstance(c, dict) and c.get("type") == "text":
                                    result_text += c.get("text", "")
                                elif isinstance(c, str):
                                    result_text += c
                        elif isinstance(block.content, str):
                            result_text = block.content
                        streamer.tool_result(block.tool_use_id, result_text)

    return "".join(final_text_parts)


async def main() -> int:
    args = parse_args(sys.argv[1:])
    prompt = args["prompt"] or sys.stdin.read()
    # If --system-prompt-file was supplied, prefer it over --system-prompt
    # (the file form is the only way to pass >128KB on Linux due to
    # MAX_ARG_STRLEN). Read once at startup.
    if args.get("system_prompt_file"):
        try:
            system_prompt = Path(args["system_prompt_file"]).read_text()
        except Exception as e:
            sys.stderr.write(f"[neil_agent] failed to read --system-prompt-file: {e}\n")
            return 1
    else:
        system_prompt = args["system_prompt"] or ""

    # Find NEIL_HOME for stream file location
    neil_home = Path(os.environ.get("NEIL_HOME", os.path.expanduser("~/.neil")))

    # Derive a prompt name from env (autoprompt.c sets this via filename guessing);
    # fall back to a timestamp
    prompt_name = os.environ.get("NEIL_PROMPT_NAME", "agent.md")

    streamer = Streamer(neil_home, prompt_name)
    exit_code = 0
    final_text = ""
    try:
        final_text = await run_agent(prompt, system_prompt, streamer)
    except Exception as e:
        streamer.text(f"\n[agent error] {e}\n")
        exit_code = 1
    finally:
        streamer.close(exit_code)

    # Print full transcript (tool activity + text) to stdout so the C daemon
    # captures it into the result file. This is what makes the command log
    # work -- the result file sees exactly what the stream file saw.
    print(streamer.get_transcript(), end="")
    return exit_code


if __name__ == "__main__":
    sys.exit(anyio.run(main))
