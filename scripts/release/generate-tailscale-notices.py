#!/usr/bin/env python3
"""Collect pinned Go module and runtime license evidence for release notices."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "native/tailscale"
NAMES = re.compile(r"^(licen[cs]e|copying|notice|copyright)(?:$|[._-])", re.I)


def objects(data):
    decoder = json.JSONDecoder()
    while data.strip():
        value, end = decoder.raw_decode(data.lstrip())
        yield value
        data = data.lstrip()[end:]


def report():
    modules = {}
    for system in ("darwin", "linux", "windows"):
        for architecture in ("amd64", "arm64"):
            environment = {**os.environ, "GOOS": system, "GOARCH": architecture, "CGO_ENABLED": "1", "GOTOOLCHAIN": "local"}
            result = subprocess.run(["go", "list", "-mod=readonly", "-deps", "-json", "."], cwd=SOURCE, env=environment, check=True, capture_output=True, text=True)
            for package in objects(result.stdout):
                module = package.get("Module")
                if module and not module.get("Main"):
                    modules[module["Path"]] = module
    sections = ["EMBEDDED TAILSCALE — GO MODULE SOURCE LICENSES\n",
                "Scope: reachable packages for macOS, Linux and Windows on amd64/arm64 (CGO enabled).",
                "go.sum SHA-256: " + hashlib.sha256((SOURCE / "go.sum").read_bytes()).hexdigest()]
    for module in sorted(modules.values(), key=lambda module: module["Path"]):
        directory = Path(module["Dir"])
        paths = {p for p in directory.iterdir() if p.is_file() and NAMES.match(p.name)}
        # Tailscale carries adapted third-party source with its own notices.
        for name in ("licenses", "LICENSES", "third_party", "tempfork", "vendor"):
            root = directory / name
            if root.is_dir():
                paths.update(p for p in root.rglob("*") if p.is_file() and NAMES.match(p.name))
        if not paths:
            raise ValueError(f"No source license evidence for {module['Path']} {module['Version']}")
        escaped = "".join("!" + c.lower() if c.isupper() else c for c in module["Path"])
        sections.append(f"\n{'=' * 78}\n{module['Path']} {module['Version']}\nSource: https://proxy.golang.org/{escaped}/@v/{module['Version']}.zip")
        for path in sorted(paths):
            text = path.read_text(encoding="utf-8")
            if not text.strip():
                raise ValueError(f"Empty source notice: {path}")
            sections.append(f"\n--- {path.relative_to(directory)} ---\n{text.rstrip()}\n")
    goroot = subprocess.run(["go", "env", "GOROOT"], check=True, capture_output=True, text=True).stdout.strip()
    license_path = Path(goroot) / "LICENSE"
    if not license_path.is_file():
        license_path = Path(goroot).parent / "LICENSE"  # Homebrew keeps notices outside libexec.
    sections.append("\nGO RUNTIME\n" + license_path.read_text())
    for name in ("MINGW-RUNTIME-LICENSE.txt", "LLVM-RUNTIME-LICENSE.txt"):
        sections.append("\nWINDOWS CGO RUNTIME — " + name + "\n" + (SOURCE / name).read_text(encoding="utf-8"))
    return "\n".join(sections)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.write_text(report(), encoding="utf-8")
