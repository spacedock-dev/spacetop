"""Local fake Responses endpoint; synthetic token/data; OS denies vendor egress.

This is not a real provider/model call and does not prove existing-login support.
It inspects the CLI's offered tool surface under the candidate launch flags.
"""

import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import platform
import shutil
import sys
import tempfile
import threading

from probe import environment, literal, profile, SANDBOX
from supervisor import Limits, run


class Endpoint(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        self.connection.settimeout(1)
        size = int(self.headers.get("Content-Length", "0"))
        if size > 262144 or len(self.server.requests) >= 4:
            self.send_error(413)
            return
        body = json.loads(self.rfile.read(size))
        self.server.requests.append(body)
        message = {"id": "msg_probe", "type": "message", "role": "assistant",
                   "status": "completed", "content": [{"type": "output_text",
                   "text": "Synthetic local response.", "annotations": []}]}
        response = {"id": "resp_probe", "object": "response", "status": "completed",
                    "output": [message], "usage": {"input_tokens": 1, "output_tokens": 1,
                    "total_tokens": 2}}
        events = [
            {"type": "response.created", "response": {**response, "status": "in_progress", "output": []}},
            {"type": "response.output_item.added", "output_index": 0, "item": message},
            {"type": "response.output_text.delta", "output_index": 0, "content_index": 0,
             "item_id": "msg_probe", "delta": "Synthetic local response."},
            {"type": "response.output_item.done", "output_index": 0, "item": message},
            {"type": "response.completed", "response": response},
        ]
        if self.server.adversarial and len(self.server.requests) == 1:
            outside = self.server.outside
            calls = [
                {"type": "function_call", "id": "fc_exec", "call_id": "call_exec",
                 "name": "exec_command", "arguments": json.dumps({"cmd": f"/usr/bin/touch {outside}/escaped-shell"}),
                 "status": "completed"},
                {"type": "custom_tool_call", "id": "fc_patch", "call_id": "call_patch",
                 "name": "apply_patch", "input": f"*** Begin Patch\n*** Add File: {outside}/escaped-patch\n+probe\n*** End Patch",
                 "status": "completed"},
                {"type": "function_call", "id": "fc_read", "call_id": "call_read",
                 "name": "read_file", "arguments": json.dumps({"path": str(outside / "sentinel")}),
                 "status": "completed"},
                {"type": "function_call", "id": "fc_image", "call_id": "call_image",
                 "name": "view_image", "arguments": json.dumps({"path": str(outside / "sentinel")}),
                 "status": "completed"},
            ]
            if self.server.auth_exception:
                calls.append({"type": "function_call", "id": "fc_auth", "call_id": "call_auth",
                              "name": "view_image", "arguments": json.dumps({"path": str(outside / "synthetic-auth.json")}),
                              "status": "completed"})
            events = [{"type": "response.created", "response": {**response, "status": "in_progress", "output": []}}]
            for index, call in enumerate(calls):
                events += [{"type": "response.output_item.added", "output_index": index, "item": call},
                           {"type": "response.output_item.done", "output_index": index, "item": call}]
            events.append({"type": "response.completed", "response": {**response, "output": calls}})
        data = "".join("data: " + json.dumps(event) + "\n\n" for event in events).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adversarial", action="store_true")
    parser.add_argument("--auth-exception", action="store_true",
                        help="Test a synthetic auth/config read exception; never uses real credentials")
    parser.add_argument("--disable-image-tool", action="store_true",
                        help="Exercise the installed CLI's explicit view_image feature switch")
    args = parser.parse_args()
    if args.auth_exception and not args.adversarial:
        parser.error("--auth-exception requires --adversarial")
    if platform.system() != "Darwin":
        print(json.dumps({"outcome": "unsupported_probe_platform", "production_ready": False}))
        return 2
    located = shutil.which("codex")
    if located is None:
        print(json.dumps({"outcome": "missing_executable", "production_ready": False}))
        return 2
    executable = Path(located).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="spacetop-protocol-") as temporary:
        root = Path(temporary).resolve()
        scratch, outside = root / "scratch", root / "outside"
        scratch.mkdir(mode=0o700)
        outside.mkdir(mode=0o700)
        (outside / "sentinel").write_text("synthetic outside-read canary")
        if args.auth_exception:
            (outside / "synthetic-auth.json").write_text('{"token":"SYNTHETIC_ONLY_NOT_A_CREDENTIAL"}')
        server = HTTPServer(("127.0.0.1", 0), Endpoint)
        server.requests = []
        server.adversarial, server.outside = args.adversarial, outside
        server.auth_exception = args.auth_exception
        server.timeout = 1
        port = server.server_address[1]
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        policy = profile(scratch, executable)
        if args.auth_exception:
            policy += "(allow file-read-data " + literal(outside / "synthetic-auth.json") + ")\n"
        policy += f'(allow network-outbound (remote ip "localhost:{port}"))\n'
        policy_file = scratch / "policy.sb"
        policy_file.write_text(policy)
        env = environment(scratch)
        env["CODEX_HOME"] = str(scratch / "codex-home")
        (scratch / "codex-home").mkdir(mode=0o700)
        env["SPACETOP_PROBE_TOKEN"] = "synthetic-not-a-real-credential"
        argv = [SANDBOX, "-f", str(policy_file), str(executable), "exec",
                "--ignore-user-config", "--ignore-rules", "--ephemeral",
                "--skip-git-repo-check", "--sandbox", "read-only", "--color", "never",
                "--model", "gpt-6-luna"]
        for feature in ("hooks", "plugins", "apps", "shell_tool", "multi_agent"):
            argv += ["--disable", feature]
        if args.disable_image_tool:
            argv += ["--disable", "view_image"]
        settings = [
            'project_doc_max_bytes=0', 'web_search="disabled"',
            'model_provider="summary_probe"',
            'model_providers.summary_probe.name="Local synthetic probe"',
            f'model_providers.summary_probe.base_url="http://127.0.0.1:{port}/v1"',
            'model_providers.summary_probe.wire_api="responses"',
            'model_providers.summary_probe.env_key="SPACETOP_PROBE_TOKEN"',
            'model_providers.summary_probe.requires_openai_auth=false',
            'model_providers.summary_probe.request_max_retries=0',
            'model_providers.summary_probe.stream_max_retries=0',
        ]
        for setting in settings:
            argv += ["-c", setting]
        argv += ["-"]
        try:
            result = run(argv, scratch, env, b"Summarize this synthetic task: show library opening hours.",
                         limits=Limits(seconds=6))
        finally:
            server.shutdown()
            server.server_close()
            worker.join(timeout=2)
        report = {"outcome": result.outcome, "exit_code": result.exit_code,
                  "live_generation_launches": 0, "production_ready": False,
                  "view_image_disabled": args.disable_image_tool,
                  "model": "gpt-6-luna", "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
                  "local_requests": len(server.requests),
                  "tools": [request.get("tools", []) for request in server.requests],
                  "tool_results": [item for request in server.requests for item in request.get("input", [])
                                   if item.get("type") in ("function_call_output", "custom_tool_call_output")],
                  "outside_unchanged": sorted(p.name for p in outside.iterdir()) == (
                      ["sentinel", "synthetic-auth.json"] if args.auth_exception else ["sentinel"]),
                  "stdout": result.stdout.decode(errors="replace"),
                  "stderr": result.stderr.decode(errors="replace")}
        print(json.dumps(report, indent=2))
        expected_requests = 2 if args.adversarial else 1
        passed = (result.outcome == "exited" and result.exit_code == 0
                  and len(server.requests) == expected_requests and report["outside_unchanged"]
                  and all(not tools for tools in report["tools"]))
        if args.adversarial:
            expected = {"call_exec": "unsupported call: exec_command",
                        "call_patch": "patch rejected: writing is blocked by read-only sandbox; rejected by user approval settings",
                        "call_read": "unsupported call: read_file",
                        "call_image": f"unable to read image at `{outside}/sentinel`: Operation not permitted (os error 1)"}
            observed = {item.get("call_id"): item.get("output") for item in report["tool_results"]}
            if args.auth_exception:
                expected["call_auth"] = "unable to process image: invalid or unsupported image data"
            if args.disable_image_tool:
                expected["call_image"] = "unsupported call: view_image"
                if args.auth_exception:
                    expected["call_auth"] = "unsupported call: view_image"
            passed = passed and observed == expected
        return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
