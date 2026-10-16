import hashlib
import importlib.util
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).resolve().parent / "server-manifest.py"
PLATKEYS = ("linux-x86_64", "linux-aarch64", "darwin-aarch64", "darwin-x86_64")


def load_module():
    spec = importlib.util.spec_from_file_location("server_manifest", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def content_for(name: str) -> bytes:
    return f"content of {name}".encode()


def write_platform(directory: pathlib.Path, key: str) -> None:
    for name in (f"vela-server-{key}", f"vela-server-{key}.zip"):
        (directory / name).write_bytes(content_for(name))
        (directory / f"{name}.sig").write_text(f"sig-of-{name}\n")


class ServerManifestTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.dir = pathlib.Path(self._tmp.name)
        for key in PLATKEYS:
            write_platform(self.dir, key)
        self.module = load_module()

    def build(self):
        return self.module.build_manifest(self.dir, "eperfecto/VelaTerm", "v0.2.15", "0.2.15")

    def test_manifest_shape(self):
        manifest = self.build()
        self.assertEqual(manifest["version"], "0.2.15")
        self.assertEqual(set(manifest["platforms"]), set(PLATKEYS))
        for key, entry in manifest["platforms"].items():
            self.assertEqual(
                set(entry), {"url", "sha256", "signature", "zip_url", "zip_sha256", "zip_signature"}
            )
            name = f"vela-server-{key}"
            self.assertEqual(
                entry["url"], f"https://github.com/eperfecto/VelaTerm/releases/download/v0.2.15/{name}"
            )
            self.assertEqual(entry["zip_url"], entry["url"] + ".zip")
            self.assertEqual(entry["sha256"], hashlib.sha256(content_for(name)).hexdigest())
            self.assertEqual(
                entry["zip_sha256"], hashlib.sha256(content_for(name + ".zip")).hexdigest()
            )
            self.assertEqual(entry["signature"], f"sig-of-{name}")
            self.assertEqual(entry["zip_signature"], f"sig-of-{name}.zip")

    def test_missing_platform_fails(self):
        for path in self.dir.glob("vela-server-darwin-x86_64*"):
            path.unlink()
        with self.assertRaises(SystemExit) as ctx:
            self.build()
        self.assertIn("darwin-x86_64", str(ctx.exception))

    def test_missing_signature_fails(self):
        (self.dir / "vela-server-linux-aarch64.zip.sig").unlink()
        with self.assertRaises(SystemExit) as ctx:
            self.build()
        self.assertIn("vela-server-linux-aarch64.zip", str(ctx.exception))

    def test_signature_read_verbatim_without_trailing_newline(self):
        (self.dir / "vela-server-linux-x86_64.sig").write_bytes(b"abc\r\n")
        manifest = self.build()
        self.assertEqual(manifest["platforms"]["linux-x86_64"]["signature"], "abc")


if __name__ == "__main__":
    unittest.main()
