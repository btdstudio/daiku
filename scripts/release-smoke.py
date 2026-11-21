#!/usr/bin/env python3
"""Smoke-test a release archive: check its checksum, unpack it, run `daiku version`
and `daiku init`.

    release-smoke.py <folder holding the archive and its .sha256> <tag> <target>

For example `release-smoke.py dl v0.3.0-alpha.2 x86_64-unknown-linux-gnu`. The release
workflow's smoke job runs it on each target's runner (S-0010); it also runs locally
against `dist build --artifacts=local` output. It uses only the standard library, so it
runs the same on Linux, macOS and Windows.
"""
import hashlib
import os
import pathlib
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile

# What `daiku init` must write into an empty folder (spec AC10).
INIT_FILES = [
    "AGENTS.md",
    "CLAUDE.md",
    "STATE.json",
    "REPOS.json",
    "sync.md",
    ".daiku/settings.json",
    ".daiku/manifest.json",
]


# Variables that can carry a credential. The binary under test hasn't passed yet, so it
# doesn't inherit them (the workflow gives only its download step a token; this is the
# second layer).
CREDENTIAL_NAMES = re.compile(
    r"^(GH_|GITHUB_|ACTIONS_|RUNNER_)|TOKEN|SECRET|PASSWORD|CREDENTIAL|API_?KEY", re.I
)


def clean_env(environ):
    """`environ` without any variable that looks like a credential."""
    return {k: v for k, v in environ.items() if not CREDENTIAL_NAMES.search(k)}


def fail(message):
    print(f"FAIL: {message}")
    sys.exit(1)


def main():
    if len(sys.argv) != 4:
        print(__doc__)
        sys.exit(2)
    folder, tag, target = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    windows = "windows" in target
    name = f"daiku-cli-{target}.{'zip' if windows else 'tar.xz'}"
    archive = folder / name
    if not archive.is_file():
        fail(f"{archive} isn't there")

    # The checksum: `<sha256> *<archive name>`, as dist writes it.
    sums = folder / f"{name}.sha256"
    if not sums.is_file():
        fail(f"{sums} isn't there")
    want, _, listed = sums.read_text().strip().partition(" ")
    if listed.lstrip("*").strip() != name:
        fail(f"{sums.name} names {listed.strip()!r}, not {name}")
    got = hashlib.sha256(archive.read_bytes()).hexdigest()
    if got != want.lower():
        fail(f"{name} hashes to {got}, its checksum file says {want}")
    print(f"ok   checksum of {name}")

    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        if windows:
            with zipfile.ZipFile(archive) as z:
                z.extractall(tmp)
        else:
            with tarfile.open(archive, "r:xz") as t:
                if hasattr(tarfile, "data_filter"):
                    # The safe filter; Python 3.14 applies it by default, and older
                    # versions warn without it.
                    t.extractall(tmp, filter="data")
                else:
                    t.extractall(tmp)
        # The Unix archives unpack into a folder named for the target; the Windows zip
        # holds its files at the top level.
        exe = "daiku.exe" if windows else "daiku"
        found = [p for p in (tmp / f"daiku-cli-{target}" / exe, tmp / exe) if p.is_file()]
        if not found:
            fail(f"the archive holds no {exe}, in daiku-cli-{target}/ or at its top level")
        binary = found[0]
        binary.chmod(0o755)
        print(f"ok   unpacked {binary.name}")

        env = clean_env(os.environ)
        env.update(
            {
                "GIT_AUTHOR_NAME": "smoke",
                "GIT_AUTHOR_EMAIL": "smoke@example.com",
                "GIT_COMMITTER_NAME": "smoke",
                "GIT_COMMITTER_EMAIL": "smoke@example.com",
            }
        )
        version = subprocess.run(
            [str(binary), "version"], capture_output=True, text=True, env=env
        )
        if version.returncode != 0 or version.stdout.strip() != f"daiku {tag}":
            fail(
                f"`daiku version` printed {version.stdout.strip()!r} "
                f"(exit {version.returncode}), want 'daiku {tag}'"
            )
        print(f"ok   daiku version is daiku {tag}")

        workspace = tmp / "workspace"
        workspace.mkdir()
        init = subprocess.run(
            [str(binary), "init"], cwd=workspace, capture_output=True, text=True, env=env
        )
        if init.returncode != 0:
            fail(f"`daiku init` exited {init.returncode}: {init.stderr.strip()}")
        missing = [f for f in INIT_FILES if not (workspace / f).is_file()]
        if missing:
            fail(f"`daiku init` wrote no {', '.join(missing)}")
        print(f"ok   daiku init wrote {len(INIT_FILES)} files")
    print(f"PASS {target} {tag}")


if __name__ == "__main__":
    main()
