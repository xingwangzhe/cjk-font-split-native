#!/usr/bin/env python3
"""Preserve Rust's PGO forced symbol through the Zig compiler driver."""
import os
import sys

args = sys.argv[1:]
forwarded = []
index = 0
while index < len(args):
    if args[index] == "-u" and index + 1 < len(args):
        forwarded.append("-Wl,-u," + args[index + 1])
        index += 2
    else:
        forwarded.append(args[index])
        index += 1
real_zig = os.environ["PGO_REAL_ZIG"]
os.execv(real_zig, [real_zig, *forwarded])
