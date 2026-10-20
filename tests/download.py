#!/usr/bin/env python3
"""Offline download trust-boundary tests; never execute unverified archive contents."""
import hashlib
import io
import json
import os
import pathlib
import shutil
import subprocess
import tarfile
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
WORK = ROOT / ".testbed/download"
WORK.mkdir(parents=True, exist_ok=True)
PINNED_VERSION = "0.9.1"
PIN = "68ef45e65ea03bdc15da7c723df0ecd778e6859efd6def8274a78c49c6dbd531"
SOURCE = "3407b2670c63b24dafe4f215081ba14d490d6543"

with tempfile.TemporaryDirectory(dir=WORK) as tmp:
    base = pathlib.Path(tmp)
    repo = base / "plugin"
    (repo / "scripts").mkdir(parents=True)
    for rel in ("scripts/install.sh", "manifest.json", "Main.qml", "ReportPanel.qml"):
        shutil.copy2(ROOT / rel, repo / rel)
    # Exercise a reviewed release even while preparing the next unpinned tag.
    manifest = json.loads((repo / "manifest.json").read_text())
    manifest["version"] = version = PINNED_VERSION
    (repo / "manifest.json").write_text(json.dumps(manifest, indent=2))
    tools = base / "tools"
    tools.mkdir()
    calls = base / "calls"
    executed = base / "executed"
    archive = base / "fixture.tar.gz"
    payload = f'#!/bin/sh\necho executed >> "{executed}"\necho "theme-report {version} fixture"\n'.encode()
    with tarfile.open(archive, "w:gz") as tar:
        info = tarfile.TarInfo(f"theme-report-{version}-linux-x86_64/bin/theme-report")
        info.mode = 0o755
        info.size = len(payload)
        tar.addfile(info, io.BytesIO(payload))

    def tool(name, body):
        path = tools / name
        path.write_text("#!/bin/bash\nset -eu\n" + body)
        path.chmod(0o755)

    tool("curl", '''echo "curl $*" >> "$CALLS"
[[ ${CURL_FAIL:-0} == 0 ]] || exit 1
while (($#)); do
  if [[ $1 == -o ]]; then cp -- "$ARCHIVE" "$2"; exit; fi
  shift
done
exit 1
''')
    # Only the fixture archive's hash is substituted, to exercise successful
    # control flow without checking a release binary into the test suite.
    # A substituted/malicious archive is tested with its REAL hash below.
    tool("sha256sum", f'''if [[ $* == *theme-report-*-linux-x86_64.tar.gz && ${{GOOD_HASH:-0}} == 1 ]]; then
  printf '%s  %s\\n' '{PIN}' "${{@: -1}}"
else
  exec /usr/bin/sha256sum "$@"
fi
''')
    tool("gh", '''echo "gh $*" >> "$CALLS"
if [[ $1 == auth ]]; then exit "${AUTH_FAIL:-0}"; fi
exit "${ATTEST_FAIL:-0}"
''')
    tool("tar", '''echo "tar $*" >> "$CALLS"
exec /usr/bin/tar "$@"
''')
    env = dict(os.environ, PATH=f"{tools}:/usr/bin:/bin", CALLS=str(calls),
               ARCHIVE=str(archive), CARGO_TARGET_DIR=str(base / "no-build"),
               TMPDIR=str(base))

    def run(name, *, success, **extra):
        calls.write_text("")
        executed.unlink(missing_ok=True)
        home = base / name
        result = subprocess.run(["bash", str(repo / "scripts/install.sh"), "--root", str(home)],
                                env=dict(env, **extra), text=True, capture_output=True)
        assert (result.returncode == 0) == success, (name, result.stdout, result.stderr)
        log = calls.read_text()
        if not success:
            assert not home.exists(), "Failed verification modified the destination"
            assert not executed.exists(), "Unverified analyzer was executed"
            assert "tar " not in log, "Unverified archive was extracted"
        return result, log

    assert hashlib.sha256(archive.read_bytes()).hexdigest() != PIN
    _, log = run("replaced-asset", success=False, AUTH_FAIL="1")
    assert "gh " not in log, "Digest must be checked before optional provenance"
    assert ".sha256" not in log, "Never trust a remotely supplied checksum"
    _, log = run("bad-provenance", success=False, GOOD_HASH="1", ATTEST_FAIL="1")
    assert f"--source-digest {SOURCE}" in log
    assert f"--source-ref refs/tags/v{version}" in log
    assert "--signer-workflow OldJobobo/theme-report/.github/workflows/release.yml" in log
    run("download-failed", success=False, CURL_FAIL="1")
    run("verified", success=True, GOOD_HASH="1")
    run("logged-out", success=True, GOOD_HASH="1", AUTH_FAIL="1")
    # No gh installed: the source-pinned digest remains mandatory and sufficient.
    (tools / "gh").unlink()
    # Restrict PATH to known tools so this works even when /usr/bin has gh.
    for name in ("bash", "dirname", "sed", "head", "uname", "cp", "rm", "mktemp",
                 "mkdir", "realpath", "grep", "cat", "mv", "cmp", "chmod", "install", "cut", "gzip"):
        (tools / name).symlink_to(shutil.which(name, path="/usr/bin:/bin"))
    env["PATH"] = str(tools)
    run("no-gh-replaced", success=False)
    run("no-gh-verified", success=True, GOOD_HASH="1")
    manifest = json.loads((repo / "manifest.json").read_text())
    manifest["version"] = "99.0.0"
    (repo / "manifest.json").write_text(json.dumps(manifest, indent=2))
    _, log = run("unreviewed-version", success=False)
    assert not log, "An unpinned version must fail before any network access"

workflow = (ROOT / ".github/workflows/release.yml").read_text()
assert "--clobber" not in workflow and "gh release upload" not in workflow
assert "gh release create" in workflow
print("PASS: pinned downloads; replaced assets; exact provenance; no gh; unknown versions; no asset replacement")
