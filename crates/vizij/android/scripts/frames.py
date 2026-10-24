"""The smoke's screenshot checks, on raw `adb exec-out screencap` frames.

    frames.py face <frame.raw>            exit 0 when the face shows
    frames.py moved <a.raw> <b.raw>       exit 0 when the face moved between them
    frames.py png <frame.raw> <out.png>   write the frame as a PNG

A raw frame is the screencap header (width, height, pixel format, and from
Android 9 a colour space: four little-endian u32 each) then RGBA rows. Both
checks look at the band between the top and bottom tenth of the screen, where
no system bar or navigation handle sits; the face is drawn over a black clear
colour. Standard library only, so it runs on any runner and any host.
"""

import struct
import sys
import zlib

# A channel above this is not the black clear colour.
LIT = 24
# The face covers at least this share of the band (Quori covers a third of a
# letterboxed phone screen).
FACE_SHARE = 0.01
# A moved face differs in at least this share of the band: Quori's eyes
# moving change 0.3 % of it on a 320x640 screen and 0.8 % on a 1080x2220
# one.
MOVED_SHARE = 0.001


def read(path):
    data = open(path, "rb").read()
    width, height = struct.unpack_from("<II", data, 0)
    header = len(data) - width * height * 4
    if header not in (12, 16):
        sys.exit(f"{path}: not a raw screencap ({len(data)} bytes for {width}x{height})")
    return width, height, memoryview(data)[header:]


def band(width, height):
    """The rows checked: the screen without its top and bottom tenth."""
    return range(height // 10, height - height // 10)


def lit(pixels, offset):
    return max(pixels[offset], pixels[offset + 1], pixels[offset + 2]) > LIT


def face(path):
    width, height, pixels = read(path)
    rows = band(width, height)
    count = sum(
        1
        for y in rows
        for x in range(0, width, 2)
        if lit(pixels, (y * width + x) * 4)
    )
    share = count / (len(rows) * ((width + 1) // 2))
    print(f"{path}: {width}x{height}, lit share {share:.4f} (face needs {FACE_SHARE})")
    return share >= FACE_SHARE


def moved(a_path, b_path):
    width, height, a = read(a_path)
    width_b, height_b, b = read(b_path)
    if (width, height) != (width_b, height_b):
        print(f"{a_path} and {b_path} differ in size")
        return True
    rows = band(width, height)
    changed = 0
    for y in rows:
        row = y * width * 4
        if a[row : row + width * 4] == b[row : row + width * 4]:
            continue
        for x in range(width):
            o = row + x * 4
            if max(abs(a[o + c] - b[o + c]) for c in range(3)) > LIT:
                changed += 1
    share = changed / (len(rows) * width)
    print(f"{a_path} -> {b_path}: {changed} pixels changed, {share:.4f} of the band (moved needs {MOVED_SHARE})")
    return share >= MOVED_SHARE


def png(path, out):
    width, height, pixels = read(path)
    rows = b"".join(
        b"\x00" + bytes(pixels[y * width * 4 : (y + 1) * width * 4]) for y in range(height)
    )

    def chunk(kind, body):
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    with open(out, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(rows, 6)))
        f.write(chunk(b"IEND", b""))
    return True


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    ok = {"face": face, "moved": moved, "png": png}[command](*args)
    sys.exit(0 if ok else 1)
