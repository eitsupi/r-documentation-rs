"""Decompress a zstd file to stdout via the system libzstd through ctypes.

Used by scan_fixture_text.sh when the zstd CLI is unavailable. Fails on
truncated final frames; concatenated frames are handled by continuing while
input remains after a frame boundary.
"""

import ctypes
import sys
from pathlib import Path

data = Path(sys.argv[1]).read_bytes()
lib = ctypes.CDLL("libzstd.so.1")


class InputBuffer(ctypes.Structure):
    _fields_ = [
        ("src", ctypes.c_void_p),
        ("size", ctypes.c_size_t),
        ("pos", ctypes.c_size_t),
    ]


class OutputBuffer(ctypes.Structure):
    _fields_ = [
        ("dst", ctypes.c_void_p),
        ("size", ctypes.c_size_t),
        ("pos", ctypes.c_size_t),
    ]


lib.ZSTD_createDStream.restype = ctypes.c_void_p
lib.ZSTD_initDStream.argtypes = [ctypes.c_void_p]
lib.ZSTD_initDStream.restype = ctypes.c_size_t
lib.ZSTD_decompressStream.argtypes = [
    ctypes.c_void_p,
    ctypes.POINTER(OutputBuffer),
    ctypes.POINTER(InputBuffer),
]
lib.ZSTD_decompressStream.restype = ctypes.c_size_t
lib.ZSTD_isError.argtypes = [ctypes.c_size_t]
lib.ZSTD_isError.restype = ctypes.c_uint

source = ctypes.create_string_buffer(data)
input_buffer = InputBuffer(ctypes.cast(source, ctypes.c_void_p), len(data), 0)
stream = lib.ZSTD_createDStream()
if not stream or lib.ZSTD_isError(lib.ZSTD_initDStream(stream)):
    raise RuntimeError("libzstd failed to initialize")
last_result = None
while input_buffer.pos < input_buffer.size:
    target = ctypes.create_string_buffer(131072)
    output_buffer = OutputBuffer(ctypes.cast(target, ctypes.c_void_p), len(target), 0)
    result = lib.ZSTD_decompressStream(
        stream, ctypes.byref(output_buffer), ctypes.byref(input_buffer)
    )
    if lib.ZSTD_isError(result):
        raise RuntimeError("libzstd failed to decompress fixture")
    last_result = result
    sys.stdout.buffer.write(target.raw[: output_buffer.pos])
if last_result != 0:
    raise RuntimeError("truncated zstd frame")
