#!/usr/bin/env python3
"""Version and stamp Shinya's macOS build without publishing a release."""
import argparse
from datetime import datetime
import json
from pathlib import Path
import plistlib
import re
import shutil


ROOT = Path(__file__).resolve().parents[2]
METADATA = ROOT / ".github/shinya-version.json"
SHA = re.compile(r"[0-9a-f]{40}")
COMPANIONS = ("Herdr", "herdr-acp", "herdr-automate", "herdr-core", "herdr-nearby", "herdr-remote-control")
LICENSES = (
    ("LICENSE", "LICENSE"),
    ("NOTICE", "NOTICE"),
    ("assets/icons/LICENSE-octicons", "LICENSE-octicons"),
    ("crates/herdr-gpui/SOUND-NOTICE.md", "SOUND-NOTICE.md"),
    ("crates/herdr-protocol/LICENSE-APACHE", "LICENSE-APACHE"),
    ("crates/herdr-protocol/NOTICE.md", "NOTICE.md"),
    ("scripts/release/herdr-core-licenses.txt", "HERDR-CORE-LICENSES.txt"),
)


def version(path):
    try:
        values = json.loads(path.read_text())
        upstream = values["upstream_version"]
        commit = values["upstream_commit"]
        revision = values["revision"]
        if not isinstance(upstream, str) or not re.fullmatch(r"[1-9][0-9]{7}\.[1-9][0-9]{0,4}", upstream):
            raise ValueError("expected YYYYMMDD.COUNTER")
        datetime.strptime(upstream.split(".")[0], "%Y%m%d")
        if not isinstance(commit, str) or not SHA.fullmatch(commit):
            raise ValueError("expected full upstream commit SHA")
        if type(revision) is not int or not 1 <= revision <= 65535:
            raise ValueError("expected a positive fork revision up to 65535")
    except (KeyError, TypeError, ValueError) as error:
        raise ValueError(f"Invalid fork version metadata: {error}") from error
    display = f"{upstream}-shinya.{revision}"
    return {
        "version": display,
        "upstream_repository": "penso/herdr-gpui",
        "upstream_version": upstream,
        "upstream_commit": commit,
        "fork_revision": revision,
        "artifact_name": f"Herdr-{display}-aarch64-apple-darwin",
    }


def stamp(args, values):
    if not SHA.fullmatch(args.commit) or args.build_number <= 0:
        raise ValueError("Expected a full fork commit SHA and positive build number")
    contents = args.bundle / "Contents"
    info_path = contents / "Info.plist"
    info = plistlib.loads(info_path.read_bytes())
    if not isinstance(info, dict):
        raise ValueError("Expected a bundle Info.plist dictionary")
    for name in COMPANIONS:
        binary = contents / "MacOS" / name
        if not binary.is_file() or binary.stat().st_size == 0:
            raise ValueError(f"Missing bundle executable: {name}")
    if not args.notices.is_file() or not args.notices.read_text().strip():
        raise ValueError("Nonempty third-party notices are required")
    for source, _ in LICENSES:
        if not (ROOT / source).is_file():
            raise ValueError(f"Missing source notice: {source}")
    metadata = {
        **values,
        "fork_commit": args.commit,
        "build_number": args.build_number,
        "target": "aarch64-apple-darwin",
    }
    info.update({
        "CFBundleShortVersionString": values["version"],
        "CFBundleVersion": str(args.build_number),
        "HerdrUpstreamVersion": values["upstream_version"],
        "HerdrUpstreamCommit": values["upstream_commit"],
        "HerdrForkCommit": args.commit,
    })
    resources = contents / "Resources"
    resources.mkdir(parents=True, exist_ok=True)
    for source, name in LICENSES:
        shutil.copyfile(ROOT / source, resources / name)
    shutil.copyfile(args.notices, resources / "THIRD-PARTY-NOTICES.txt")
    encoded = json.dumps(metadata, indent=2) + "\n"
    (resources / "BUILD-INFO.json").write_text(encoded)
    info_path.write_bytes(plistlib.dumps(info))
    args.output_info.parent.mkdir(parents=True, exist_ok=True)
    args.output_info.write_text(encoded)
    return metadata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    read = commands.add_parser("version")
    read.add_argument("--metadata", type=Path, default=METADATA)
    read.add_argument("--github-output", type=Path)
    package = commands.add_parser("stamp")
    package.add_argument("--metadata", type=Path, default=METADATA)
    package.add_argument("--bundle", type=Path, required=True)
    package.add_argument("--notices", type=Path, required=True)
    package.add_argument("--build-number", type=int, required=True)
    package.add_argument("--commit", required=True)
    package.add_argument("--output-info", type=Path, required=True)
    args = parser.parse_args()
    try:
        values = version(args.metadata)
        if args.command == "stamp":
            values = stamp(args, values)
        elif args.github_output:
            with args.github_output.open("a") as output:
                for key in ("version", "upstream_commit", "artifact_name"):
                    output.write(f"{key}={values[key]}\n")
    except (OSError, ValueError, plistlib.InvalidFileException) as error:
        parser.error(str(error))
    print(json.dumps(values, indent=2))


if __name__ == "__main__":
    main()
