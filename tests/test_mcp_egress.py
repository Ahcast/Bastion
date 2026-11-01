import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("mcp_egress", Path(__file__).resolve().parents[1] / "deploy" / "bastion-mcp-egress.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class GuardTest(unittest.TestCase):
    def test_adds_only_missing_rules_and_is_idempotent(self):
        installed = set()
        def run(args, **kwargs):
            operation = args[3]
            key = (args[0], tuple(args[6:] if operation == "-I" else args[5:]))
            if operation == "-C":
                return SimpleNamespace(returncode=0 if key in installed else 1)
            if operation == "-I":
                installed.add(key)
                return SimpleNamespace(returncode=0)
            if operation == "-D":
                installed.remove(key)
                return SimpleNamespace(returncode=0)
            raise AssertionError(args)
        module.install(1234, run)
        self.assertEqual(len(installed), sum(map(len, module.NETWORKS.values())))
        module.install(1234, run)
        self.assertEqual(len(installed), sum(map(len, module.NETWORKS.values())))
        self.assertTrue(all("443" in rule and "1234" in rule for _, rule in installed))
        module.remove(1234, run)
        self.assertEqual(len(installed), 0)
        module.remove(1234, run)

    def test_rolls_back_only_rules_added_by_failed_attempt(self):
        operations = []
        def run(args, **kwargs):
            operation = args[3]
            operations.append(operation)
            if operation == "-C": return SimpleNamespace(returncode=1)
            if operation == "-I": return SimpleNamespace(returncode=1 if operations.count("-I") == 2 else 0)
            if operation == "-D": return SimpleNamespace(returncode=0)
            raise AssertionError(args)
        with self.assertRaises(RuntimeError): module.install(1234, run)
        self.assertEqual(operations.count("-D"), 1)

    def test_persistent_ufw_rules_are_first_and_preserve_existing_rules(self):
        for binary, chain in [("iptables", "ufw-before-output"), ("ip6tables", "ufw6-before-output")]:
            source = "*filter\n:" + chain + " - [0:0]\n-A " + chain + " -j ACCEPT\nCOMMIT\n"
            rendered = module.render_ufw(source, 1234, binary)
            output_rules = [line for line in rendered.splitlines() if line.startswith("-A " + chain + " ")]
            self.assertEqual(output_rules[:-1], module.persistent_lines(1234, binary))
            self.assertEqual(output_rules[-1], "-A " + chain + " -j ACCEPT")
            self.assertEqual(rendered.count(module.MARKER), len(module.NETWORKS[binary]))
            with self.assertRaises(ValueError): module.render_ufw(rendered, 1234, binary)
            with self.assertRaises(ValueError): module.render_ufw("invalid", 1234, binary)
            with self.assertRaises(ValueError): module.render_ufw(source.replace("COMMIT", "-I " + chain + " -j ACCEPT\nCOMMIT"), 1234, binary)

    def test_startup_rejects_missing_rules_or_inactive_ufw(self):
        contents = {}
        for binary, chain, filename in [("iptables", "ufw-before-output", "/etc/ufw/before.rules"), ("ip6tables", "ufw6-before-output", "/etc/ufw/before6.rules")]:
            original = "*filter\n:" + chain + " - [0:0]\n-A " + chain + " -j ACCEPT\nCOMMIT\n"
            contents[filename] = module.render_ufw(original, 1234, binary)
        class FakePath:
            def __init__(self, filename): self.filename = filename
            def read_text(self): return contents[self.filename]
        active = SimpleNamespace(returncode=0, stdout="Status: active\n")
        with patch.object(module, "Path", FakePath), patch.object(module.subprocess, "run", return_value=active):
            module.verify_ufw(1234)
            with self.assertRaises(RuntimeError): module.verify_ufw(4321)
        with patch.object(module, "Path", FakePath), patch.object(module.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout="Status: inactive\n")):
            with self.assertRaises(RuntimeError): module.verify_ufw(1234)

    def test_rejects_root(self):
        with self.assertRaises(ValueError): module.install(0)


if __name__ == "__main__": unittest.main()
