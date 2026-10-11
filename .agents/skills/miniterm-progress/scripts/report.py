"""Send bounded UTF-8 task reports to the inherited MiniTerm instance."""
import argparse
import json
import os
import subprocess
import sys
import uuid

MAX_BYTES = 48 * 1024


def run_helper(executable, arguments, payload=None):
    result = subprocess.run(
        [executable, *arguments], input=payload, capture_output=True, timeout=8,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )
    if result.returncode:
        # The CLI emits fixed validation errors, never echoes the input report.
        raise ValueError("MiniTerm rejected the report: " + result.stderr.decode("utf-8", "replace").strip()[:400])
    try:
        return json.loads(result.stdout.decode("utf-8"))
    except (ValueError, UnicodeError) as error:
        raise ValueError("MiniTerm helper returned an unsupported response.") from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("begin", "update", "read", "clear"))
    parser.add_argument("--work-id")
    parser.add_argument("--revision", type=int)
    args = parser.parse_args()
    session_id = os.environ.get("MINITERM_SESSION_ID", "")
    executable = os.environ.get("MINITERM_NOTIFY_EXE", "")
    try:
        uuid.UUID(session_id)
        if not os.path.isabs(executable) or not os.path.isfile(executable):
            raise ValueError("Inherited MiniTerm helper is unavailable.")
        # 'sessions' exists in older releases. Never send an unknown CLI command
        # to them: their fallback would activate the user's existing window.
        capabilities = run_helper(executable, ["sessions"])
        if (not isinstance(capabilities, dict) or not capabilities.get("ok")
                or "progress-v1" not in capabilities.get("capabilities", [])
                or "progress-v1" not in capabilities.get("helperCapabilities", [])
                or session_id not in capabilities.get("sessions", [])):
            raise ValueError("This MiniTerm terminal does not support progress-v1.")
        command = ["progress", args.action]
        if args.action in ("update", "clear"):
            uuid.UUID(args.work_id or "")
            if args.revision is None or args.revision < 2:
                raise ValueError("Provide the next revision (previous + 1).")
            command.extend([args.work_id, str(args.revision)])
        elif args.work_id is not None or args.revision is not None:
            raise ValueError("Receipt arguments apply only to update and clear.")
        payload = None
        if args.action in ("begin", "update"):
            payload = sys.stdin.buffer.read(MAX_BYTES + 1)
            if len(payload) > MAX_BYTES:
                raise ValueError("Report exceeds 48 KiB.")
            value = json.loads(payload.decode("utf-8"))
            if not isinstance(value, dict):
                raise ValueError("Report must be a JSON object.")
        response = run_helper(executable, command, payload)
        print(json.dumps(response, ensure_ascii=True, separators=(",", ":")))
        return 0
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f"MiniTerm progress: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
