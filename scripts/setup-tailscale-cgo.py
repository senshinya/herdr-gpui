#!/usr/bin/env python3
"""Install the checksum-pinned LLVM MinGW used by embedded Tailscale on Windows.

Print its bin directory (suitable for GITHUB_PATH). macOS is supported for
cross-compiling the Windows ABI without changing the host's C compiler.
"""
import argparse
import hashlib
from pathlib import Path
import platform
import tarfile
import tempfile
import urllib.request
import zipfile

VERSION = "20261020"
ASSETS = {
    "aarch64.zip": "8f24fedd0b75c674c6c83dec2d76a7ac31a690120212843a17e465ef49854f69",
    "x86_64.zip": "fd5462b7f5d93b2ff981fcd3f9c7e427e47f3203c64ea91c27bcc444eebd74cd",
    "macos-universal.tar.xz": "e9c13e4ede38e715003ffc674d4d00a1088df8235439d1f0c16188ff0704c17e",
}


def install(destination):
    if platform.system() == "Darwin":
        host = "macos-universal.tar.xz"
    elif platform.system() == "Windows":
        host = "aarch64.zip" if platform.machine().lower() in ("arm64", "aarch64") else "x86_64.zip"
    else:
        raise ValueError("Use the host GCC on Linux; this installer supports Windows and macOS")
    name = f"llvm-mingw-{VERSION}-ucrt-{host}"
    directory = destination / name.removesuffix(".tar.xz").removesuffix(".zip")
    if not directory.is_dir():
        destination.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=destination) as temp:
            archive = Path(temp) / name
            url = f"https://github.com/mstorsjo/llvm-mingw/releases/download/{VERSION}/{name}"
            with urllib.request.urlopen(url, timeout=120) as response, archive.open("wb") as output:
                while data := response.read(1024 * 1024):
                    output.write(data)
            if hashlib.file_digest(archive.open("rb"), "sha256").hexdigest() != ASSETS[host]:
                raise ValueError("LLVM MinGW archive digest mismatch")
            if host.endswith(".zip"):
                with zipfile.ZipFile(archive) as contents:
                    contents.extractall(destination)
            else:
                with tarfile.open(archive) as contents:
                    contents.extractall(destination, filter="data")
    if not (directory / "bin").is_dir():
        raise ValueError("LLVM MinGW archive has no compiler directory")
    return (directory / "bin").resolve()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    print(install(parser.parse_args().destination))
