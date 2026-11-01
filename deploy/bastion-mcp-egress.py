#!/usr/bin/env python3
"""Install narrow, idempotent egress guards for the Bastion service UID.

Only TCP/443 to infrastructure/private ranges is denied. Loopback is left
alone because other Bastion components may use local services. The MCP
fetcher independently rejects loopback and pins validated DNS addresses.
"""
import subprocess
import sys

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
        while True:
            check = run([command, "-w", "5", "-C", "OUTPUT", *rule], capture_output=True, timeout=10)
            if check.returncode == 1:
                break
            if check.returncode != 0:
                raise RuntimeError("could not inspect egress guard")
            result = run([command, "-w", "5", "-D", "OUTPUT", *rule], capture_output=True, timeout=10)
            if result.returncode:
                raise RuntimeError("could not remove egress guard")

def main():
    if len(sys.argv) == 1:
        install(service_uid())
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
