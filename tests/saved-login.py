#!/usr/bin/env python3
"""Save and reuse a synthetic password in the real GTK flow, without host access."""
import argparse
from pathlib import Path
import tempfile
from screenshots import demo_environment, run_demo

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/vpn-manager')
args = parser.parse_args()
with tempfile.TemporaryDirectory(prefix='vpn-manager-saved-login-') as temporary:
    folder = Path(temporary)
    tool = folder / 'secret-tool'
    tool.write_text('''#!/usr/bin/env python3
import hashlib, pathlib, sys
assert 'synthetic-saved-password' not in sys.argv
operation = sys.argv[1]
attributes = sys.argv[3:] if operation == 'store' else sys.argv[2:]
key = hashlib.sha256('\\0'.join(attributes).encode()).hexdigest()
path = pathlib.Path(__file__).with_name('secret-' + key)
if operation == 'lookup':
    if not path.exists(): sys.exit(1)
    sys.stdout.write(path.read_text())
elif operation == 'store':
    secret = sys.stdin.read()
    assert secret == 'synthetic-saved-password'
    path.write_text(secret)
    path.chmod(0o600)
    counter = pathlib.Path(__file__).with_name('vault-writes')
    counter.write_text(str(int(counter.read_text()) + 1 if counter.exists() else 1))
elif operation == 'clear':
    path.unlink(missing_ok=True)
else:
    sys.exit(2)
''')
    tool.chmod(0o700)
    with demo_environment(folder) as base_env:
        env = {**base_env, 'VPN_MANAGER_TEST_SAVED_LOGIN': '1',
               'VPN_MANAGER_TEST_ACTIVATION': str(folder / 'synthetic-activation')}
        result = run_demo([str(args.binary.resolve()), '--self-test'], env, timeout=60)
        if result.returncode or 'SAVED_LOGIN_OK' not in result.stdout:
            raise SystemExit(result.stdout + result.stderr)
        saved = [path for path in folder.glob('secret-*') if path != tool]
        assert len(saved) == 1 and saved[0].read_text() == 'synthetic-saved-password', 'Saving and cancelling a retry must preserve the original vault item'
        assert (folder / 'vault-writes').read_text() == '1', 'One-time mode, automatic reuse and cancelled login must not save credentials'
        assert not (folder / 'synthetic-activation').exists(), 'Synthetic connection must be stopped'
        print('SAVED_LOGIN_OK: one-time mode does not save; default mode saves and reuses; failed retry cancelled; no host keyring or tunnel accessed')
