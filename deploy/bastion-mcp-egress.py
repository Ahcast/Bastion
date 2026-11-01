#!/usr/bin/env python3
"""Install a dedicated nftables egress guard without changing UFW policy."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

TABLE = "bastion_mcp"
MARKER = "bastion-mcp-egress-v2"
STATE = Path("/run/bastion-mcp-egress-state.json")
IPV4 = ("0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "169.254.0.0/16", "172.16.0.0/12", "192.168.0.0/16")
IPV6 = ("fc00::/7", "fe80::/10")


def service_uid():
    import pwd
    result = subprocess.run(["systemctl", "show", "bastion.service", "--property=User", "--value"], capture_output=True, text=True, check=True, timeout=10)
    name = result.stdout.strip()
    if not name or name == "root":
        raise RuntimeError("Bastion must use a dedicated non-root service user")
    uid = pwd.getpwnam(name).pw_uid
    if uid == 0:
        raise RuntimeError("Bastion service UID must not be zero")
    return uid


def ruleset(uid):
    if not isinstance(uid, int) or uid <= 0:
        raise ValueError("invalid service UID")
    return ("create table inet " + TABLE + " { comment \"" + MARKER + "\"; }\n"
            "add chain inet " + TABLE + " output_guard { type filter hook output priority -5; policy accept; }\n"
            "add rule inet " + TABLE + " output_guard meta skuid " + str(uid) + " tcp dport 443 ip daddr { " + ", ".join(IPV4) + " } counter reject\n"
            "add rule inet " + TABLE + " output_guard meta skuid " + str(uid) + " tcp dport 443 ip6 daddr { " + ", ".join(IPV6) + " } counter reject\n")


def current(run=subprocess.run):
    result = run(["nft", "-n", "-s", "list", "table", "inet", TABLE], capture_output=True, timeout=10)
    if result.returncode == 0:
        return result.stdout
    # A separate list operation distinguishes a missing table from permission
    # or subsystem failures. Never overwrite an unknown table.
    listing = run(["nft", "-j", "list", "tables"], capture_output=True, timeout=10)
    if listing.returncode:
        raise RuntimeError("could not inspect nftables")
    data = json.loads(listing.stdout)
    if any(item.get("table", {}).get("family") == "inet" and item.get("table", {}).get("name") == TABLE for item in data.get("nftables", [])):
        raise RuntimeError("could not inspect existing MCP nftables table")
    return None


def read_state(path=STATE):
    if not path.exists():
        return None
    info = path.lstat()
    if path.is_symlink() or info.st_uid != 0 or info.st_mode & 0o077:
        raise RuntimeError("unsafe MCP egress state file")
    return json.loads(path.read_text())


def valid_table(uid, text):
    try:
        lines = [line.strip() for line in text.decode("utf8").splitlines() if line.strip()]
    except UnicodeDecodeError:
        return False
    if len(lines) != 8 or lines[:4] != [
        "table inet " + TABLE + " {",
        "comment \"" + MARKER + "\"",
        "chain output_guard {",
        "type filter hook output priority -5; policy accept;",
    ] or lines[-2:] != ["}", "}"]:
        return False
    prefix = "meta skuid " + str(uid) + " tcp dport 443 "
    v4 = prefix + "ip daddr { " + ", ".join(IPV4) + " } counter reject"
    v6 = prefix + "ip6 daddr { " + ", ".join(IPV6) + " } counter reject"
    return lines[4].startswith(v4) and lines[5].startswith(v6)


def matches(uid, text, state):
    return (state is not None and state.get("marker") == MARKER and state.get("uid") == uid
            and valid_table(uid, text) and state.get("sha256") == hashlib.sha256(text).hexdigest())


def install(uid, run=subprocess.run, state_path=STATE):
    document = ruleset(uid).encode()
    existing = current(run)
    state = read_state(state_path)
    if existing is not None:
        if not matches(uid, existing, state):
            raise RuntimeError("existing MCP nftables table differs from recorded state")
        return
    if state is not None:
        raise RuntimeError("MCP nftables table is missing despite recorded state")
    for command in (["nft", "--check", "--file", "-"], ["nft", "--file", "-"]):
        result = run(command, input=document, capture_output=True, timeout=10)
        if result.returncode:
            raise RuntimeError("could not install MCP nftables guard")
    try:
        text = current(run)
        if text is None or not valid_table(uid, text):
            raise RuntimeError("installed MCP nftables table could not be verified")
        state_path.parent.mkdir(parents=True, exist_ok=True)
        fd, temp = tempfile.mkstemp(prefix=".mcp-egress-", dir=str(state_path.parent))
        try:
            with os.fdopen(fd, "w") as out:
                json.dump({"marker": MARKER, "uid": uid, "sha256": hashlib.sha256(text).hexdigest()}, out)
                out.flush(); os.fsync(out.fileno())
            os.chmod(temp, 0o600)
            # Refuse an unexpected state file rather than replacing it.
            os.link(temp, state_path)
        finally:
            if os.path.exists(temp): os.unlink(temp)
    except Exception:
        # Only the table created in this invocation may be removed here.
        rollback = run(["nft", "delete", "table", "inet", TABLE], capture_output=True, timeout=10)
        if rollback.returncode:
            raise RuntimeError("MCP egress state failed; table rollback requires inspection") from None
        raise


def remove(uid, run=subprocess.run, state_path=STATE):
    existing = current(run)
    state = read_state(state_path)
    if existing is None and state is None:
        return
    if existing is None or not matches(uid, existing, state):
        raise RuntimeError("refusing to remove an unverified MCP nftables table")
    result = run(["nft", "delete", "table", "inet", TABLE], capture_output=True, timeout=10)
    if result.returncode:
        raise RuntimeError("could not remove MCP nftables guard")
    state_path.unlink()


def main():
    uid = service_uid()
    if len(sys.argv) == 1:
        install(uid)
        print("Bastion MCP egress guard installed")
    elif sys.argv[1:] == ["--remove"]:
        remove(uid)
        print("Bastion MCP egress guard removed")
    else:
        raise ValueError("unexpected arguments")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
