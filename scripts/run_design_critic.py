#!/usr/bin/env python3
"""Run a fresh, filesystem-isolated judge and immediately submit its verdict.

The critic receives only brief.txt and explicitly attached anonymous images.
Its filesystem contains system runtime files, a fresh home with authentication,
these packet files, its output schema, and its own initially empty output folder.
The repository, referee secrets, prior verdicts, and provenance logs are absent.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parent.parent
MODEL = "gpt-6-astra"
DEFAULT_MODEL = "fable"
DEADLINE = dt.datetime.fromisoformat(json.loads((ROOT / 'progress/state.json').read_text())['deadline'].replace('Z', '+00:00')).timestamp()
DISABLED = (
    "shell_tool", "unified_exec", "apps", "plugins", "remote_plugin", "memories",
    "multi_agent", "multi_agent_v2", "view_image", "browser_use",
    "browser_use_external", "browser_use_full_cdp_access", "computer_use",
    "hooks", "code_mode_host", "code_mode", "image_generation", "goals",
    "sleep_tool", "skill_mcp_dependency_install", "skill_search", "tool_suggest",
    "in_app_browser", "shell_snapshot", "workspace_dependencies",
)
SCHEMA = {
    "type": "object",
    "properties": {
        "choice": {"type": "string", "enum": ["A", "B"]},
        "evidence_A": {"type": "string", "minLength": 1},
        "evidence_B": {"type": "string", "minLength": 1},
        "loser_reason": {"type": "string", "minLength": 1},
    },
    "required": ["choice", "evidence_A", "evidence_B", "loser_reason"],
    "additionalProperties": False,
}


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def timestamp() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def packet_input(packet: Path, model: str = MODEL) -> tuple[str, list[Path]]:
    if not packet.is_dir() or packet.parent != (ROOT / "judging/design-blind").resolve():
        raise ValueError("Packet must be a direct child of judging/design-blind")
    brief_path = packet / "brief.txt"
    if brief_path.is_symlink() or not brief_path.is_file():
        raise ValueError("Packet needs a regular brief.txt")
    brief = brief_path.read_text()
    names = ["A.png", "B.png"]
    # Numbered panels are allowed only when explicitly listed in this brief.
    extra = re.findall(r"\b[AB]-?\d+\.png\b", brief)
    names.extend(name for name in extra if name not in names)
    images = []
    for name in names:
        path = packet / name
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Missing or symbolic image: {name}")
        images.append(path)
    prompt = (
        "You are a fresh independent visual design critic. You have no prior "
        "conversation, identities, authorship, scores, or answer key. Judge only "
        "the visible supplied images against the exact question and criteria "
        "in the packet brief below. Inspect each attached image. Do not invoke "
        "tools, identify the products, infer functionality that is not visible, "
        "or seek other context. Make the forced A or B choice required by the "
        "brief. Return only the required JSON object; concrete visible evidence "
        "for BOTH entries and the single biggest reason the loser lost are "
        "required. No ties or scores.\n\n"
        "Attachment order: " + ", ".join(names) + ".\n\n"
        "BEGIN PACKET BRIEF\n" + brief + "\nEND PACKET BRIEF\n"
    )
    if model == "fable":
        prompt = prompt.replace(
            "Inspect each attached image. Do not invoke tools, identify the products,",
            "Use Read to inspect EVERY supplied image at /packet/<filename>, including numbered panels. "
            "Only Read is available, confined to this packet. Do not identify the products,",
        )
    return prompt, images


def isolated_command(images: list[Path], schema: Path, output: Path) -> list[str]:
    executable = shutil.which("codex")
    if not executable or not shutil.which("bwrap"):
        raise ValueError("Both codex and bwrap are required; isolation has no fallback")
    executable = str(Path(executable).resolve())
    user_home = Path.home()
    auth = user_home / ".codex/auth.json"
    if not auth.is_file():
        raise ValueError("Existing Codex authentication file is unavailable")
    cmd = [
        "bwrap", "--unshare-user", "--unshare-pid", "--unshare-ipc", "--unshare-uts",
        "--die-with-parent", "--new-session", "--ro-bind", "/usr", "/usr",
        "--ro-bind", "/etc", "/etc", "--symlink", "usr/bin", "/bin",
        "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
        "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp",
        "--dir", str(user_home / ".codex"),
        "--ro-bind", str(auth), str(auth),
        "--ro-bind", executable, "/opt/codex",
        "--dir", "/packet", "--ro-bind", str(schema), "/schema.json",
        "--bind", str(output), "/output",
    ]
    # resolv.conf is a symlink into /run on this host; bind its exact target.
    resolver = Path("/etc/resolv.conf").resolve()
    if not str(resolver).startswith("/etc/"):
        cmd.extend(["--ro-bind", str(resolver), str(resolver)])
    for image in images:
        cmd.extend(["--ro-bind", str(image), "/packet/" + image.name])
    cmd.extend([
        "--clearenv", "--setenv", "HOME", str(user_home),
        "--setenv", "PATH", "/usr/bin:/bin", "--setenv", "LANG", "C.UTF-8",
        "--chdir", "/packet", "/opt/codex", "exec",
        "--model", MODEL, "--sandbox", "read-only", "--ephemeral",
        "--ignore-user-config", "--ignore-rules", "--skip-git-repo-check",
        "--color", "never", "--output-schema", "/schema.json",
        "--output-last-message", "/output/final.json",
        "-c", "model_reasoning_effort=high", "-c", "approval_policy=never",
        "-c", 'web_search="disabled"',
        "-c", "suppress_unstable_features_warning=true",
    ])
    for feature in DISABLED:
        cmd.extend(["--disable", feature])
    cmd.extend(["--enable", "skip_host_skill_discovery"])
    for image in images:
        cmd.extend(["--image", "/packet/" + image.name])
    cmd.append("-")
    return cmd


def fable_command(images: list[Path], brief: Path | None, output: Path, config: Path, schema: dict) -> list[str]:
    executable = shutil.which("claude")
    if not executable or not shutil.which("bwrap"):
        raise ValueError("Both claude and bwrap are required; isolation has no fallback")
    user_home = Path.home()
    auth = user_home / ".claude/.credentials.json"
    if not auth.is_file():
        raise ValueError("Existing Claude OAuth authentication file is unavailable")
    cmd = [
        "bwrap", "--unshare-user", "--unshare-pid", "--unshare-ipc", "--unshare-uts",
        "--die-with-parent", "--new-session", "--ro-bind", "/usr", "/usr",
        "--ro-bind", "/etc", "/etc", "--symlink", "usr/bin", "/bin",
        "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
        "--dev", "/dev", "--proc", "/proc", "--tmpfs", "/tmp",
        "--dir", str(user_home / ".claude"), "--ro-bind", str(auth), str(auth),
        "--ro-bind", str(config), str(user_home / ".claude.json"),
        "--ro-bind", str(Path(executable).resolve()), "/opt/claude",
        "--dir", "/packet", "--bind", str(output), "/output",
    ]
    resolver = Path("/etc/resolv.conf").resolve()
    if not str(resolver).startswith("/etc/"):
        cmd.extend(["--ro-bind", str(resolver), str(resolver)])
    for image in images:
        cmd.extend(["--ro-bind", str(image), "/packet/" + image.name])
    if brief:
        cmd.extend(["--ro-bind", str(brief), "/packet/brief.txt"])
    cmd.extend([
        "--clearenv", "--setenv", "HOME", str(user_home),
        "--setenv", "PATH", "/usr/bin:/bin", "--setenv", "LANG", "C.UTF-8",
        "--chdir", "/packet", "/opt/claude", "-p", "--model", "fable", "--effort", "high",
        "--safe-mode", "--restricted", "--tools", "Read", "--allowedTools", "Read",
        "--permission-mode", "dontAsk", "--no-session-persistence",
        "--strict-mcp-config", "--mcp-config", '{"mcpServers":{}}',
        "--output-format", "stream-json", "--verbose", "--json-schema", json.dumps(schema),
    ])
    return cmd


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", nargs="?", type=Path)
    parser.add_argument("--model", choices=[MODEL, "fable"], default=DEFAULT_MODEL)
    parser.add_argument("--probe", action="store_true", help="Check isolated Astra access without judging")
    parser.add_argument("--timeout", type=float, default=180, help="Per-critic seconds, capped at hard deadline")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("timeout must be positive")
    if time.time() >= DEADLINE:
        raise SystemExit("Frozen at user deadline")
    if args.probe:
        prompt = 'Return exactly {"probe":"ready"}. Do not use tools.'
        images = []
        packet = None
        schema_value = {"type": "object", "properties": {"probe": {"type": "string", "enum": ["ready"]}}, "required": ["probe"], "additionalProperties": False}
        label = "probe"
    else:
        if args.packet is None:
            parser.error("packet is required unless --probe is used")
        packet = args.packet.resolve()
        prompt, images = packet_input(packet, args.model)
        schema_value = SCHEMA
        label = packet.name
    logs = ROOT / ".runtime/design-critics"
    logs.mkdir(parents=True, exist_ok=True)
    run = logs / (label + "-" + uuid.uuid4().hex[:12])
    run.mkdir(mode=0o700)
    log_path = run / "critic.log"
    provenance_path = run / "provenance.json"
    provenance = {
        "packet": packet.name if packet else None,
        "requested_model": args.model,
        "reasoning_effort": "high",
        "fresh_ephemeral_session": True,
        "started": timestamp(),
        "deadline": dt.datetime.fromtimestamp(DEADLINE, dt.timezone.utc).isoformat(),
        "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
        "images": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in images},
        "isolation": "bwrap: repository, prior logs and referee keys not mounted; only packet-confined Read for Fable, tool features disabled for Astra",
        "status": "starting",
    }
    write_json(provenance_path, provenance)
    (run / "prompt.txt").write_text(prompt)
    with tempfile.TemporaryDirectory(prefix="spacemap-astra-") as temp_name:
        temp = Path(temp_name)
        schema = temp / "schema.json"
        write_json(schema, schema_value)
        output = temp / "output"
        output.mkdir()
        if args.model == "fable":
            config = temp / "claude-auth-config.json"
            original_config = json.loads((Path.home() / ".claude.json").read_text())
            clean_config = {"hasCompletedOnboarding": True}
            if "oauthAccount" in original_config:
                clean_config["oauthAccount"] = original_config["oauthAccount"]
            write_json(config, clean_config)
            cmd = fable_command(images, packet / "brief.txt" if packet else None, output, config, schema_value)
        else:
            cmd = isolated_command(images, schema, output)
        provenance["argv"] = cmd
        write_json(provenance_path, provenance)
        timeout = min(args.timeout, DEADLINE - time.time())
        if timeout <= 0:
            raise SystemExit("Frozen at user deadline")
        with log_path.open("w") as log:
            proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
            try:
                proc.communicate(prompt, timeout=timeout)
            except subprocess.TimeoutExpired:
                grace = min(3, max(0, DEADLINE - time.time()))
                os.killpg(proc.pid, signal.SIGTERM if grace else signal.SIGKILL)
                try:
                    proc.wait(timeout=grace)
                except subprocess.TimeoutExpired:
                    os.killpg(proc.pid, signal.SIGKILL)
                    proc.wait()
                provenance.update(status="timeout", finished=timestamp())
                write_json(provenance_path, provenance)
                print(json.dumps({"status": "timeout", "log": str(log_path)}), flush=True)
                return 124
        transcript = log_path.read_text()
        actual_model = re.search(r"(?m)^model:\s*(\S+)", transcript)
        session = re.search(r"(?m)^session id:\s*([a-f0-9-]+)", transcript)
        provenance.update(exit_code=proc.returncode, finished=timestamp(), effective_model=actual_model.group(1) if actual_model else None, session_id=session.group(1) if session else None)
        final_path = output / "final.json"
        try:
            if proc.returncode:
                raise ValueError(f"Codex exited {proc.returncode}")
            if args.model == "fable":
                events = []
                for line in transcript.splitlines():
                    try:
                        events.append(json.loads(line))
                    except json.JSONDecodeError:
                        continue
                result = next((event for event in reversed(events) if event.get("type") == "result"), None)
                if result is None:
                    raise ValueError("Fable execution log has no final result")
                if result.get("is_error"):
                    raise ValueError("Fable reported an error")
                models = list(result.get("modelUsage", {}))
                if not models or any("fable" not in name for name in models):
                    raise ValueError("Execution log did not attest Fable model usage")
                if not result.get("session_id"):
                    raise ValueError("Execution log did not attest a fresh session ID")
                provenance.update(effective_model=", ".join(models), session_id=result["session_id"])
                image_reads = {}
                successful_reads = set()
                for event in events:
                    for part in event.get("message", {}).get("content", []):
                        if isinstance(part, dict) and part.get("type") == "tool_use" and part.get("name") == "Read":
                            image_reads[part["id"]] = part.get("input", {}).get("file_path", "")
                        if isinstance(part, dict) and part.get("type") == "tool_result" and not part.get("is_error"):
                            successful_reads.add(part.get("tool_use_id"))
                read_paths = {os.path.normpath(os.path.join("/packet", path)) for tool_id, path in image_reads.items() if tool_id in successful_reads}
                expected_paths = {"/packet/" + path.name for path in images}
                if not expected_paths.issubset(read_paths):
                    raise ValueError("Fable did not successfully inspect every packet image")
                provenance["verified_image_reads"] = sorted(expected_paths)
                verdict = result.get("structured_output")
                if verdict is None:
                    verdict = json.loads(result["result"])
            else:
                if not actual_model or actual_model.group(1) != MODEL:
                    raise ValueError("Execution log did not attest the required model")
                if not session:
                    raise ValueError("Execution log did not attest a fresh session ID")
                verdict = json.loads(final_path.read_text())
            if args.probe:
                if verdict != {"probe": "ready"}:
                    raise ValueError("Availability probe returned an unexpected response")
            else:
                if set(verdict) != set(SCHEMA["required"]) or verdict["choice"] not in {"A", "B"}:
                    raise ValueError("Invalid verdict shape")
                if any(not isinstance(verdict[k], str) or not verdict[k].strip() for k in SCHEMA["required"]):
                    raise ValueError("Verdict evidence must be nonempty text")
            verdict_path = run / "verdict.json"
            write_json(verdict_path, verdict)
            provenance["status"] = "valid"
            write_json(provenance_path, provenance)
        except (ValueError, OSError) as error:
            provenance.update(status="invalid", error=str(error))
            write_json(provenance_path, provenance)
            print(json.dumps({"status": "invalid", "error": str(error), "log": str(log_path)}), flush=True)
            return 1
    if not args.probe:
        if time.time() >= DEADLINE:
            provenance["status"] = "late-not-submitted"
            write_json(provenance_path, provenance)
            return 124
        critic = f"{provenance['effective_model']} / {provenance['session_id']}"
        try:
            submitted = subprocess.run([sys.executable, str(ROOT / "judging/design-submit.py"), "--packet", packet.name, "--verdict", str(verdict_path), "--critic", critic], cwd=ROOT, timeout=max(.001, min(15, DEADLINE - time.time())))
        except subprocess.TimeoutExpired:
            provenance["status"] = "submission-timeout"
            write_json(provenance_path, provenance)
            return 124
        provenance["submission_exit_code"] = submitted.returncode
        provenance["status"] = "submitted" if submitted.returncode == 0 else "submission-failed"
        write_json(provenance_path, provenance)
        if submitted.returncode:
            return submitted.returncode
    print(json.dumps({"status": provenance["status"], "model": provenance["effective_model"], "session": provenance["session_id"], "verdict": verdict, "log": str(log_path), "provenance": str(provenance_path)}), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
