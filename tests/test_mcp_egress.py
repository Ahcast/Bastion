import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest

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

    def test_rejects_root(self):
        with self.assertRaises(ValueError): module.install(0)


if __name__ == "__main__": unittest.main()
