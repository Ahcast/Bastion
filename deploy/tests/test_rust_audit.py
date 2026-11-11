import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    "audit", Path(__file__).parents[1] / "check_rust_audit.py"
)
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class RustAuditPolicy(unittest.TestCase):
    def setUp(self):
        self.report = {"vulnerabilities": {"found": True, "count": 1, "list": [{
            "advisory": {"id": "RUSTSEC-2026-0459"},
            "package": {"name": "libsqlite3-sys", "version": "0.37.0"},
        }]}}
        self.tree = "bastion-server v0.1.0 (/workspace/server)\nsqlx v0.9.0 (*)\n"

    def test_only_inactive_optional_sqlite_is_excluded(self):
        rejected, excluded = audit.assess(self.report, self.tree)
        self.assertEqual(rejected, [])
        self.assertEqual(len(excluded), 1)

    def test_enabling_sqlite_fails(self):
        self.tree += "libsqlite3-sys v0.37.0\n"
        rejected, excluded = audit.assess(self.report, self.tree)
        self.assertEqual(len(rejected), 1)
        self.assertEqual(excluded, [])

    def test_other_advisory_and_version_fail(self):
        finding = self.report["vulnerabilities"]["list"][0]
        finding["advisory"]["id"] = "RUSTSEC-2026-0385"
        self.assertEqual(len(audit.assess(self.report, self.tree)[0]), 1)
        finding["advisory"]["id"] = "RUSTSEC-2026-0459"
        finding["package"]["version"] = "0.36.0"
        self.assertEqual(len(audit.assess(self.report, self.tree)[0]), 1)

    def test_incomplete_tree_fails(self):
        with self.assertRaises(ValueError):
            audit.assess(self.report, "")

    def test_unrecognized_tree_line_fails(self):
        with self.assertRaises(ValueError):
            audit.assess(self.report, self.tree + "unexpected output")

    def test_inconsistent_report_fails(self):
        self.report["vulnerabilities"]["list"] = []
        with self.assertRaises(ValueError):
            audit.assess(self.report, self.tree)

    def test_clean_report_passes(self):
        self.report["vulnerabilities"] = {"found": False, "count": 0, "list": []}
        self.assertEqual(audit.assess(self.report, self.tree), ([], []))


if __name__ == "__main__":
    unittest.main()
