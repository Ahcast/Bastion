#!/usr/bin/env python3
"""Synthetic PDF renderer for MCP confirmation tests. No network or email."""
from pathlib import Path
import sys

assert sys.argv[1] == "compile"
Path(sys.argv[-1]).write_bytes(b"%PDF-1.4\nsynthetic MCP confirmation test\n%%EOF")
