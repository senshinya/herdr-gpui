"""Windows source installs carry the matching native DLL beside the CLI."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "install-windows.py"

class InstallTests(unittest.TestCase):
    def test_runtime_is_installed_beside_the_executable(self):
        spec = importlib.util.spec_from_file_location("install_windows", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            build = root / "build"
            build.mkdir()
            library = build / "herdr_tailscale.dll"
            library.write_bytes(b"matching native ABI")
            destination = root / "cargo install root" / "bin"
            destination.mkdir(parents=True)
            (destination / "herdr-gpui.exe").write_bytes(b"app")
            module.copy_runtime(library, destination)
            self.assertEqual((destination / library.name).read_bytes(), library.read_bytes())
            library.unlink()
            with self.assertRaises(FileNotFoundError):
                module.copy_runtime(library, destination)
            self.assertEqual((destination / "herdr_tailscale.dll").read_bytes(), b"matching native ABI")
