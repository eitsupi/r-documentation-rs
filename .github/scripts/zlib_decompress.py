"""Decompress a raw zlib stream from stdin to stdout.

Used by scan_fixture_text.sh for .rdbentry payloads (the four-byte size
prefix is stripped by the caller).
"""

import sys
import zlib

sys.stdout.buffer.write(zlib.decompress(sys.stdin.buffer.read()))
