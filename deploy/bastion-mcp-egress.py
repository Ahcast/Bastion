#!/usr/bin/env python3
"""Install narrow, idempotent egress guards for the Bastion service UID.

Only TCP/443 to infrastructure/private ranges is denied. Loopback is left
alone because other Bastion components may use local services. The MCP
fetcher independently rejects loopback and pins validated DNS addresses.
"""
import os
import subprocess
import sys
from pathlib import Path

MARKER = "bastion-mcp-egress-v1"
NETWORKS = {
    "iptables": ("0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "169.254.0.0/16", "172.16.0.0/12", "192.168.0.0/16"),
    "ip6tables": ("fc00::/7", "fe80::/10"),
}


def service_uid():
    import pwd
    result = subprocess.run(
        ["systemctl", "show", "bastion.service", "--property=User", "--value"],
        capture_output=True, text=True, check=True, timeout=10,
    )
    name = result.stdout.strip()
    if not name or name == "root":
        raise RuntimeError("Bastion must use a dedicated non-root service user")
    uid = pwd.getpwnam(name).pw_uid
    if uid == 0:
        raise RuntimeError("Bastion service UID must not be zero")
    return uid


def rules(uid):
    if not isinstance(uid, int) or uid <= 0:
        raise ValueError("invalid service UID")
    for command, networks in NETWORKS.items():
        for network in networks:
            yield command, ["-p", "tcp", "-d", network, "--dport", "443", "-m", "owner", "--uid-owner", str(uid), "-m", "comment", "--comment", MARKER, "-j", "REJECT"]


def persistent_lines(uid, binary):
    chain = "ufw-before-output" if binary == "iptables" else "ufw6-before-output"
    return ["-A " + chain + " " + " ".join(rule) for command, rule in rules(uid) if command == binary]


def unsafe_chain_mutation(lines, chain):
    for line in lines:
        tokens = line.strip().split()
        if not tokens:
            continue
        if tokens[0] in ("-I", "--insert", "-R", "--replace", "-D", "--delete", "-F", "--flush", "-X", "--delete-chain"):
            if len(tokens) == 1 or tokens[1] == chain:
                return True
    return False


def render_ufw(original, uid, binary):
    if binary not in NETWORKS or MARKER in original:
        raise ValueError("unexpected UFW configuration")
    lines = original.splitlines()
    chain = "ufw-before-output" if binary == "iptables" else "ufw6-before-output"
    if unsafe_chain_mutation(lines, chain):
        raise ValueError("unsafe UFW chain mutation")
    if lines.count("*filter") != 1 or lines.count("COMMIT") != 1 or not any(line.startswith(":" + chain + " ") for line in lines):
        raise ValueError("unexpected UFW filter structure")
    positions = [i for i, line in enumerate(lines) if line.startswith("-A ")]
    if not positions or positions[0] >= lines.index("COMMIT"):
        raise ValueError("unexpected UFW filter ordering")
    lines[positions[0]:positions[0]] = persistent_lines(uid, binary)
    return "\n".join(lines) + "\n"


def verify_ufw(uid):
    for binary, filename in (("iptables", "/etc/ufw/before.rules"), ("ip6tables", "/etc/ufw/before6.rules")):
        text = Path(filename).read_text()
        chain = "ufw-before-output" if binary == "iptables" else "ufw6-before-output"
        actual = [line.strip() for line in text.splitlines() if line.strip().startswith("-A " + chain + " ")]
        expected = persistent_lines(uid, binary)
        if unsafe_chain_mutation(text.splitlines(), chain) or actual[:len(expected)] != expected or sum(MARKER in line for line in text.splitlines()) != len(expected):
            raise RuntimeError("UFW persistent MCP rules are missing or incorrect")
    result = subprocess.run(["ufw", "status"], capture_output=True, text=True, timeout=10, env={**os.environ, "LC_ALL": "C"})
    if result.returncode or not any(line.strip() == "Status: active" for line in result.stdout.splitlines()):
        raise RuntimeError("UFW must be active for MCP egress protection")


def install(uid, run=subprocess.run):
    added = []
    try:
        for command, rule in rules(uid):
            check = run([command, "-w", "5", "-C", "OUTPUT", *rule], capture_output=True, timeout=10)
            if check.returncode == 0:
                continue
            if check.returncode != 1:
                raise RuntimeError("could not inspect egress guard")
            result = run([command, "-w", "5", "-I", "OUTPUT", "1", *rule], capture_output=True, timeout=10)
            if result.returncode:
                raise RuntimeError("could not install egress guard")
            added.append((command, rule))
    except Exception:
        rollback_failed = False
        for command, rule in reversed(added):
            try:
                result = run([command, "-w", "5", "-D", "OUTPUT", *rule], capture_output=True, timeout=10)
                rollback_failed |= result.returncode != 0
            except Exception:
                rollback_failed = True
        if rollback_failed:
            raise RuntimeError("egress guard failed and partial rollback requires inspection") from None
        raise


def remove(uid, run=subprocess.run):
    # Remove only exact rules owned by this guard; never flush a chain.
    for command, rule in rules(uid):
        chain = "ufw-before-output" if command == "iptables" else "ufw6-before-output"
        for target in ("OUTPUT", chain):
            while True:
                check = run([command, "-w", "5", "-C", target, *rule], capture_output=True, timeout=10)
                if check.returncode == 1:
                    break
                if check.returncode != 0:
                    raise RuntimeError("could not inspect egress guard")
                result = run([command, "-w", "5", "-D", target, *rule], capture_output=True, timeout=10)
                if result.returncode:
                    raise RuntimeError("could not remove egress guard")

def main():
    if len(sys.argv) == 1:
        uid = service_uid()
        verify_ufw(uid)
        install(uid)
        print("Bastion MCP egress guards installed")
    elif sys.argv[1:] == ["--remove"]:
        remove(service_uid())
        print("Bastion MCP egress guards removed")
    else:
        raise ValueError("unexpected arguments")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
