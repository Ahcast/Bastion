import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("mcp_egress", Path(__file__).resolve().parents[1] / "deploy" / "bastion-mcp-egress.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FakeNft:
    def __init__(self):
        self.table = None
        self.operations = []
        self.fail_after_create = False

    def __call__(self, command, **kwargs):
        self.operations.append(command)
        if command[1:5] == ["-n", "-s", "list", "table"]:
            if self.table is None or self.fail_after_create:
                return SimpleNamespace(returncode=1, stdout=b"")
            return SimpleNamespace(returncode=0, stdout=self.table)
        if command[1:] == ["-j", "list", "tables"]:
            tables = [{"table": {"family": "inet", "name": module.TABLE}}] if self.table else []
            return SimpleNamespace(returncode=0, stdout=json.dumps({"nftables": tables}).encode())
        if command[1:] == ["--check", "--file", "-"]:
            return SimpleNamespace(returncode=0, stdout=b"")
        if command[1:] == ["--file", "-"]:
            uid = "1234"
            self.table = ("table inet " + module.TABLE + " {\n"
                + 'comment "' + module.MARKER + '"\n'
                + "chain output_guard {\ntype filter hook output priority -5; policy accept;\n"
                + "meta skuid " + uid + " tcp dport 443 ip daddr { " + ", ".join(module.IPV4) + " } counter reject with icmp 3\n"
                + "meta skuid " + uid + " tcp dport 443 ip6 daddr { " + ", ".join(module.IPV6) + " } counter reject with icmpv6 4\n}\n}\n").encode()
            return SimpleNamespace(returncode=0, stdout=b"")
        if command[1:] == ["delete", "table", "inet", module.TABLE]:
            self.table = None
            return SimpleNamespace(returncode=0, stdout=b"")
        raise AssertionError(command)


class GuardTest(unittest.TestCase):
    def test_rules_are_uid_scoped_and_preserve_loopback(self):
        text = module.ruleset(1234)
        self.assertIn("meta skuid 1234 tcp dport 443", text)
        self.assertIn("169.254.0.0/16", text)
        self.assertIn("fc00::/7", text)
        self.assertNotIn("127.0.0.0/8", text)
        self.assertNotIn("::1/128", text)
        self.assertFalse(module.valid_table(1234, b'table inet bastion_mcp {\ncomment "bastion-mcp-egress-v2"\n}\n'))
        with self.assertRaises(ValueError): module.ruleset(0)

    def test_install_is_idempotent_and_remove_is_selective(self):
        fake = FakeNft()
        with TemporaryDirectory(dir=Path(__file__).resolve().parent) as directory:
            state = Path(directory) / "state.json"
            with patch.object(module, "read_state", side_effect=lambda path: json.loads(path.read_text()) if path.exists() else None), patch.object(module.os, "link", side_effect=lambda src, dst: Path(dst).write_bytes(Path(src).read_bytes())):
                module.install(1234, fake, state)
                first = fake.table
                module.install(1234, fake, state)
                self.assertEqual(fake.table, first)
                self.assertEqual(sum(command[1:] == ["--file", "-"] for command in fake.operations), 1)
                module.remove(1234, fake, state)
                self.assertIsNone(fake.table)
                self.assertFalse(state.exists())
                module.remove(1234, fake, state)

    def test_unknown_or_modified_table_is_never_removed(self):
        fake = FakeNft()
        with TemporaryDirectory(dir=Path(__file__).resolve().parent) as directory:
            state = Path(directory) / "state.json"
            with patch.object(module, "read_state", side_effect=lambda path: json.loads(path.read_text()) if path.exists() else None), patch.object(module.os, "link", side_effect=lambda src, dst: Path(dst).write_bytes(Path(src).read_bytes())):
                fake.table = b"unknown"
                with self.assertRaises(RuntimeError): module.install(1234, fake, state)
                self.assertEqual(fake.table, b"unknown")
                fake.table = None
                module.install(1234, fake, state)
                fake.table += b" modified"
                with self.assertRaises(RuntimeError): module.remove(1234, fake, state)
                self.assertIsNotNone(fake.table)

    def test_failed_verification_rolls_back_only_new_table(self):
        fake = FakeNft(); fake.fail_after_create = True
        with TemporaryDirectory(dir=Path(__file__).resolve().parent) as directory:
            state = Path(directory) / "state.json"
            with patch.object(module, "read_state", return_value=None):
                with self.assertRaises(RuntimeError): module.install(1234, fake, state)
            self.assertIsNone(fake.table)
            self.assertFalse(state.exists())


if __name__ == "__main__": unittest.main()
