#!/usr/bin/env python3
"""Build/install Windows programs and their in-process Tailscale runtime."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ("herdr-gpui", "herdr-gpui-launcher", "herdr-acp", "herdr-automate", "herdr-remote-control")


def copy_runtime(library, destination):
    if not library.is_file():
        raise FileNotFoundError(f"The build did not produce {library}")
    # A failed copy leaves the existing DLL intact. Windows refuses replacing
    # a loaded DLL, so stop the installed app before updating it.
    with tempfile.NamedTemporaryFile(dir=destination, prefix=".tailscale-", delete=False) as output:
        temporary = Path(output.name)
    try:
        shutil.copyfile(library, temporary)
        temporary.replace(destination / library.name)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    target_dir = Path(metadata["target_directory"])
    version = subprocess.check_output(["rustc", "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
    if "windows" not in host:
        raise ValueError("This source installer is for a Windows host")
    install_root = Path(os.environ.get("CARGO_INSTALL_ROOT") or os.environ.get("CARGO_HOME") or Path.home() / ".cargo")
    for package in PACKAGES:
        command = ["cargo", "install", "--locked", "--force", "--target", host, "--target-dir", str(target_dir), "--root", str(install_root), "--path", str(ROOT / "crates" / package)]
        if package == "herdr-gpui":
            command.extend(["--bin", "herdr-gpui"])
        subprocess.run(command, cwd=ROOT, check=True)
        if package == "herdr-gpui":
            copy_runtime(target_dir / host / "release/herdr_tailscale.dll", install_root / "bin")
    subprocess.run([str(install_root / "bin/herdr-gpui.exe"), "--version"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
