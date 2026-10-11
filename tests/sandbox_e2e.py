#!/usr/bin/env python3
"""Assert the kernel sandbox actually contains the process it wraps.

The probe (tests/sandbox_probe.c) performs eleven checks and reports how many
failed. It also tries to open an outbound connection to 127.0.0.1:4173 and
compares the result against the profile's network mode, so this test starts
its own listener: without one the connection is refused in 'all' mode too,
and the suite reports a sandbox escape that did not happen.
"""

import json
import http.server
import os
import pathlib
import socketserver
import subprocess
import tempfile
import threading

BIN = os.environ.get('DELTA_BIN', 'target/debug/delta')

# The probe hardcodes this port, so the listener has to match it.
PROBE_PORT = 4173


class _Quiet(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'ok')

    def log_message(self, *args):
        pass


class _Server(socketserver.TCPServer):
    allow_reuse_address = True


def main():
    server = _Server(('127.0.0.1', PROBE_PORT), _Quiet)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    results = []
    try:
        with tempfile.TemporaryDirectory(prefix='delta-kernel-test-') as td:
            d = pathlib.Path(td)
            (d / 'read.txt').write_text('allowed')
            (d / 'secret.txt').write_text('must remain unchanged')
            (d / 'writes').mkdir()
            subprocess.run(['cc', 'tests/sandbox_probe.c', '-pthread', '-o', str(d / 'probe')], check=True)

            for mode in ['all', 'none']:
                policy = {
                    'name': 'test-' + mode,
                    'read': [p for p in ['/usr', '/lib', '/lib64', '/etc/ld.so.cache', str(d / 'read.txt')]
                             if pathlib.Path(p).exists()],
                    'write': [str(d / 'writes')],
                    'network': mode,
                    'allow_child_processes': False,
                    'cpu_seconds': 20,
                    'max_open_files': 128,
                }
                (d / 'policy.json').write_text(json.dumps(policy))
                args = [BIN, 'sandbox', 'exec', '--policy', str(d / 'policy.json'), '--',
                        str(d / 'probe'), str(d / 'read.txt'), str(d / 'secret.txt'),
                        str(d / 'writes' / ('output-' + mode)), str(int(mode == 'all'))]
                p = subprocess.run(args, env={**os.environ, 'DELTA_TOKEN': 'must-not-leak'},
                                   text=True, capture_output=True, timeout=30)
                assert p.returncode == 0, (p.stdout, p.stderr)
                result = json.loads(p.stdout)
                assert result['failed'] == 0, result
                assert result['outbound_connected'] == (mode == 'all'), result
                assert (d / 'secret.txt').read_text() == 'must remain unchanged'
                assert (d / 'read.txt').read_text() == 'allowed'
                results.append({'profile': mode, 'result': result})

            out = pathlib.Path(os.environ.get('DELTA_EVIDENCE_DIR', 'artifacts/v04'))
            out.mkdir(parents=True, exist_ok=True)
            (out / 'sandbox-results.json').write_text(json.dumps({
                'results': results,
                'checks_per_profile': 11,
                'platform': os.uname().sysname,
            }, indent=2))
    finally:
        server.shutdown()
        server.server_close()
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
