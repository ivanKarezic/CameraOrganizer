import struct
import zlib
from pathlib import Path

def png(width: int, height: int, pixels: bytes) -> bytes:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + pixels[y * width * 4 : (y + 1) * width * 4] for y in range(height))
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)),
            chunk(b"IDAT", zlib.compress(raw, 9)),
            chunk(b"IEND", b""),
        ]
    )

size = 1024
pixels = bytearray(size * size * 4)
bg = (18, 14, 10, 255)
amber = (232, 154, 42, 255)
cream = (232, 220, 200, 255)

for y in range(size):
    for x in range(size):
        i = (y * size + x) * 4
        pixels[i : i + 4] = bytes(bg)
        # sprocket holes
        if 70 < x < 150 or 874 < x < 954:
            cy = ((y + 40) % 160) - 80
            if cy * cy + (x - (110 if x < 512 else 914)) ** 2 < 28 * 28:
                pixels[i : i + 4] = bytes((8, 6, 4, 255))
        # inner frame
        if 190 < x < 834 and 190 < y < 834:
            if x < 210 or x > 814 or y < 210 or y > 814:
                pixels[i : i + 4] = bytes(amber)
            elif 230 < x < 794 and 230 < y < 794:
                pixels[i : i + 4] = bytes((28, 22, 16, 255))

# simple C mark
for y in range(360, 664):
    for x in range(360, 664):
        dx, dy = x - 512, y - 512
        r2 = dx * dx + dy * dy
        if 90 * 90 < r2 < 128 * 128 and not (dx > 40 and abs(dy) < 48):
            i = (y * size + x) * 4
            pixels[i : i + 4] = bytes(cream)

out = Path("/Users/kare/Projects/Personal/CameraOrganizer/app-icon.png")
out.write_bytes(png(size, size, bytes(pixels)))
print(f"wrote {out} ({out.stat().st_size} bytes)")
