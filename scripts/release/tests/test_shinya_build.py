"""Exercise the fork's version and bundle metadata without building an app."""
import json
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "shinya-build.py"
UPSTREAM = "a" * 40
COMMIT = "b" * 40


class ShinyaBuildTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="herdr-fork-build-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.metadata = self.root / "version.json"
        self.values = {"upstream_version": "20261028.1", "upstream_commit": UPSTREAM, "revision": 1}
        self.write_metadata()

    def write_metadata(self):
        self.metadata.write_text(json.dumps(self.values))

    def run_script(self, command, *args):
        return subprocess.run(
            [sys.executable, str(SCRIPT), command, "--metadata", str(self.metadata), *args],
            capture_output=True, text=True, timeout=10,
        )

    def test_version_identifies_the_upstream_release_and_fork_revision(self):
        result = self.run_script("version")
        self.assertEqual(result.returncode, 0, result.stderr)
        result = json.loads(result.stdout)
        self.assertEqual(result["version"], "20261028.1-shinya.1")
        self.assertEqual(result["upstream_commit"], UPSTREAM)
        self.assertEqual(result["artifact_name"], "Herdr-20261028.1-shinya.1-aarch64-apple-darwin")

    def test_invalid_metadata_cannot_become_an_output_or_filename(self):
        for key, value in [("revision", True), ("revision", 0), ("revision", -1),
                           ("upstream_version", "20260230.1"),
                           ("upstream_version", "20261028.1\nother=value"),
                           ("upstream_commit", "main")]:
            with self.subTest(key=key, value=value):
                previous = self.values[key]
                self.values[key] = value
                self.write_metadata()
                result = self.run_script("version")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Invalid fork version metadata", result.stderr)
                self.assertEqual(result.stdout, "")
                self.values[key] = previous

    def test_github_outputs_are_validated_single_line_values(self):
        output = self.root / "github-output"
        result = self.run_script("version", "--github-output", str(output))
        self.assertEqual(result.returncode, 0, result.stderr)
        outputs = dict(line.split("=", 1) for line in output.read_text().splitlines())
        self.assertEqual(outputs["version"], "20261028.1-shinya.1")
        self.assertEqual(outputs["upstream_commit"], UPSTREAM)

    def bundle(self):
        bundle = self.root / "Herdr.app"
        resources = bundle / "Contents/Resources"
        resources.mkdir(parents=True)
        executables = bundle / "Contents/MacOS"
        executables.mkdir()
        for name in ("Herdr", "herdr-acp", "herdr-automate", "herdr-core", "herdr-nearby", "herdr-remote-control"):
            (executables / name).write_bytes(b"test executable")
        info = bundle / "Contents/Info.plist"
        info.write_bytes(plistlib.dumps({"CFBundleIdentifier": "so.pen.herdr-gpui.dev"}))
        notices = self.root / "notices.txt"
        notices.write_text("fixture third-party notices")
        return bundle, info, notices

    def test_bundle_and_build_info_keep_the_exact_upstream_and_fork_commits(self):
        bundle, info, notices = self.bundle()
        output = self.root / "build-info.json"
        result = self.run_script(
            "stamp", "--bundle", str(bundle), "--notices", str(notices),
            "--build-number", "42", "--commit", COMMIT, "--output-info", str(output),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        values = plistlib.loads(info.read_bytes())
        self.assertEqual(values["CFBundleShortVersionString"], "20261028.1-shinya.1")
        self.assertEqual(values["CFBundleVersion"], "42")
        self.assertEqual(values["CFBundleIdentifier"], "so.pen.herdr-gpui.dev")
        metadata = json.loads(output.read_text())
        self.assertEqual(metadata["upstream_commit"], UPSTREAM)
        self.assertEqual(metadata["fork_commit"], COMMIT)
        resources = bundle / "Contents/Resources"
        self.assertEqual(json.loads((resources / "BUILD-INFO.json").read_text()), metadata)
        self.assertEqual((resources / "THIRD-PARTY-NOTICES.txt").read_text(), notices.read_text())
        self.assertTrue((resources / "HERDR-CORE-LICENSES.txt").is_file())

    def test_empty_notices_leave_the_bundle_unchanged(self):
        bundle, info, notices = self.bundle()
        before = info.read_bytes()
        notices.write_text("")
        result = self.run_script(
            "stamp", "--bundle", str(bundle), "--notices", str(notices),
            "--build-number", "42", "--commit", COMMIT,
            "--output-info", str(self.root / "build-info.json"),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(info.read_bytes(), before)

    def test_missing_core_refuses_to_package_an_incomplete_bundle(self):
        bundle, info, notices = self.bundle()
        before = info.read_bytes()
        (bundle / "Contents/MacOS/herdr-core").unlink()
        output = self.root / "build-info.json"
        result = self.run_script(
            "stamp", "--bundle", str(bundle), "--notices", str(notices),
            "--build-number", "42", "--commit", COMMIT, "--output-info", str(output),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing bundle executable: herdr-core", result.stderr)
        self.assertEqual(info.read_bytes(), before)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
