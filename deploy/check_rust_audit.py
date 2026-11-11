"""Audit the lockfile, with one fail-closed exception for unused SQLite."""

import json
import re
import subprocess
import sys


def assess(report, tree):
    packages = set()
    for line in tree.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"([A-Za-z0-9_-]+) v(\d+\.\d+\.\d+[^ ]*)(?: .*)?", line)
        if not match:
            raise ValueError("Unrecognized cargo tree output")
        packages.add(match.groups())
    active_names = {name for name, version in packages}
    if "bastion-server" not in active_names:
        raise ValueError("Incomplete workspace dependency graph")
    vulnerabilities = report["vulnerabilities"]
    findings = vulnerabilities["list"]
    if not isinstance(findings, list) or vulnerabilities["count"] != len(findings):
        raise ValueError("Invalid vulnerability report")
    if vulnerabilities["found"] != bool(findings):
        raise ValueError("Inconsistent vulnerability report")
    rejected, excluded = [], []
    for finding in findings:
        advisory = finding["advisory"]["id"]
        package = finding["package"]
        label = f'{advisory}: {package["name"]} {package["version"]}'
        # SQLx 0.9 locks optional SQLite even with default features disabled.
        # Its <0.38 constraint cannot select the >=0.39 fix. Never exempt an
        # active SQLite dependency or a different advisory/version.
        if (
            advisory == "RUSTSEC-2026-0459"
            and package["name"] == "libsqlite3-sys"
            and package["version"] == "0.37.0"
            and "libsqlite3-sys" not in active_names
            and "sqlx-sqlite" not in active_names
            and ("sqlx", "0.9.0") in packages
        ):
            excluded.append(label)
        else:
            rejected.append(label)
    return rejected, excluded


def main():
    audit = subprocess.run(
        ["cargo", "audit", "--json"], capture_output=True, text=True, timeout=180
    )
    if audit.returncode not in (0, 1):
        raise RuntimeError(f"cargo audit failed: {audit.stderr[-2000:]}")
    # cargo metadata includes inactive weak optional dependencies; cargo tree
    # applies feature unification. Include all platforms and all edge kinds
    # used by builds/tests, with the same workspace features as CI.
    tree = subprocess.run(
        ["cargo", "tree", "--locked", "--workspace", "--target", "all",
         "--edges", "normal,build,dev", "--prefix", "none", "--format", "{p}"],
        capture_output=True, text=True, timeout=180, check=True,
    )
    report = json.loads(audit.stdout)
    rejected, excluded = assess(report, tree.stdout)
    if audit.returncode == 1 and not report["vulnerabilities"]["found"]:
        raise RuntimeError(f"cargo audit failed without findings: {audit.stderr[-2000:]}")
    for label in excluded:
        print(f"INACTIVE OPTIONAL DEPENDENCY (absent from cargo tree): {label}")
    for label in rejected:
        print(f"REJECTED: {label}", file=sys.stderr)
    if rejected:
        return 1
    print("Dependency audit passed; any inactive exception is listed above.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (KeyError, TypeError, ValueError, RuntimeError, OSError,
            subprocess.SubprocessError) as error:
        print(f"Dependency audit could not be verified: {error}", file=sys.stderr)
        sys.exit(1)
