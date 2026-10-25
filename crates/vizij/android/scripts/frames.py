"""The smoke's screenshot checks, on raw `adb exec-out screencap` frames.

    frames.py face <frame.raw>            exit 0 when the face shows
    frames.py moved <a.raw> <b.raw>       exit 0 when the face moved between them
    frames.py replaced <a.raw> <b.raw>    exit 0 when another face replaced it
    frames.py find <frame.raw> RRGGBB     print where the colour shows, as "x y";
                                          exit 0 when it shows
    frames.py background <frame.raw> RRGGBB
                                          exit 0 when the colour fills the
                                          background
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
# A face replaced by another differs in at least this share of the band
# (Toasty over Quori, about 40 %).
REPLACED_SHARE = 0.05
# How far, per channel, a pixel may be from a colour sought and be it: the
# settings pane's fills, drawn opaque, land within a step or two of their
# value.
NEAR = 3
# A colour sought shows when at least this many pixels are it: a button or a
# swatch of the settings pane is hundreds of them, a stray match a few.
SHOWN_PIXELS = 200
# A background fills at least this share of the band: the face, letterboxed
# on a phone, covers about a third of it.
BACKGROUND_SHARE = 0.25


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


def changed_pixels(a_path, b_path):
    """The pixels of the band that differ between two frames, and the band's
    size; `None` for frames of different sizes."""
    width, height, a = read(a_path)
    width_b, height_b, b = read(b_path)
    if (width, height) != (width_b, height_b):
        return None
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
    return changed, len(rows) * width


def moved(a_path, b_path):
    counted = changed_pixels(a_path, b_path)
    if counted is None:
        print(f"{a_path} and {b_path} differ in size")
        return True
    changed, total = counted
    share = changed / total
    print(f"{a_path} -> {b_path}: {changed} pixels changed, {share:.4f} of the band (moved needs {MOVED_SHARE})")
    return share >= MOVED_SHARE


def replaced(a_path, b_path):
    counted = changed_pixels(a_path, b_path)
    if counted is None:
        print(f"{a_path} and {b_path} differ in size")
        return False
    changed, total = counted
    share = changed / total
    print(f"{a_path} -> {b_path}: {share:.4f} of the band changed (replaced needs {REPLACED_SHARE})")
    return share >= REPLACED_SHARE


def near(pixels, offset, rgb):
    return all(abs(pixels[offset + c] - rgb[c]) <= NEAR for c in range(3))


def rgb_of(hex_colour):
    hex_colour = hex_colour.lstrip("#")
    return [int(hex_colour[i : i + 2], 16) for i in (0, 2, 4)]


def find(path, hex_colour):
    """Where `hex_colour` shows on the whole screen: the centre of the pixels
    that are it, printed as "x y" for `adb shell input tap`."""
    width, height, pixels = read(path)
    rgb = rgb_of(hex_colour)
    xs = ys = count = 0
    for y in range(0, height, 2):
        for x in range(0, width, 2):
            if near(pixels, (y * width + x) * 4, rgb):
                xs += x
                ys += y
                count += 1
    # Every other row and column is sampled: a quarter of the pixels.
    shown = count * 4 >= SHOWN_PIXELS
    if shown:
        print(f"{xs // count} {ys // count}")
    print(f"{path}: #{hex_colour} on about {count * 4} pixels", file=sys.stderr)
    return shown


def background(path, hex_colour):
    width, height, pixels = read(path)
    rgb = rgb_of(hex_colour)
    rows = band(width, height)
    count = sum(
        1
        for y in rows
        for x in range(0, width, 2)
        if near(pixels, (y * width + x) * 4, rgb)
    )
    share = count / (len(rows) * ((width + 1) // 2))
    print(f"{path}: #{hex_colour} on {share:.4f} of the band (a background needs {BACKGROUND_SHARE})")
    return share >= BACKGROUND_SHARE


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
    ok = {
        "face": face,
        "moved": moved,
        "replaced": replaced,
        "find": find,
        "background": background,
        "png": png,
    }[command](*args)
    sys.exit(0 if ok else 1)
