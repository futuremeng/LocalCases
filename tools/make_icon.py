#!/usr/bin/env python3
"""Write desktop/icons/icon.png (512x512 RGBA) without any image library.

Tauri refuses to bundle without an icon, and a placeholder beats a build that
fails on first run. Replace it with real artwork before shipping.
"""
import pathlib
import struct
import zlib

SIZE = 512
BG = (31, 95, 191, 255)
FG = (255, 255, 255, 255)


def shade(x: int, y: int) -> tuple:
    # rounded corners
    r = 96
    if x < r and y < r and (r - x) ** 2 + (r - y) ** 2 > r * r:
        return (0, 0, 0, 0)
    if x > SIZE - r and y < r and (x - (SIZE - r)) ** 2 + (r - y) ** 2 > r * r:
        return (0, 0, 0, 0)
    if x < r and y > SIZE - r and (r - x) ** 2 + (y - (SIZE - r)) ** 2 > r * r:
        return (0, 0, 0, 0)
    if x > SIZE - r and y > SIZE - r and (x - (SIZE - r)) ** 2 + (y - (SIZE - r)) ** 2 > r * r:
        return (0, 0, 0, 0)
    # a magnifier: ring + handle
    cx, cy, ring = 216, 216, 112
    if (cx - x) ** 2 + (cy - y) ** 2 <= ring**2 and (cx - x) ** 2 + (cy - y) ** 2 >= (ring - 26) ** 2:
        return FG
    if 300 <= x <= 404 and 300 <= y <= 404 and abs(x - y) <= 26:
        return FG
    return BG


def main() -> None:
    raw = bytearray()
    for y in range(SIZE):
        raw.append(0)  # filter type: none
        for x in range(SIZE):
            raw.extend(shade(x, y))
    out = pathlib.Path("desktop/icons/icon.png")
    out.parent.mkdir(parents=True, exist_ok=True)

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
    out.write_bytes(png)
    print(f"wrote {out} ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
