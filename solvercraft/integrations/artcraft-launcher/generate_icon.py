"""Create Solvercraft's original geometric launcher icon; no external assets."""
from pathlib import Path
import math
import struct
import zlib

HERE = Path(__file__).resolve().parent
SIZE = 128
SCALE = 4
BACKGROUND = (15, 31, 45, 255)
AXES = (132, 166, 183, 255)
ACCENT = (67, 212, 194, 255)
POINTS = [(28, 85), (49, 63), (72, 73), (100, 35)]


def distance(x, y, start, end):
    ax, ay = start
    bx, by = end
    dx, dy = bx - ax, by - ay
    t = max(0, min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(x - ax - t * dx, y - ay - t * dy)


def sample(x, y):
    cx, cy = min(112, max(16, x)), min(112, max(16, y))
    if math.hypot(x - cx, y - cy) > 12:
        return (0, 0, 0, 0)
    color = BACKGROUND
    if min(distance(x, y, (27, 29), (27, 100)), distance(x, y, (27, 100), (103, 100))) <= 1.25:
        color = AXES
    if any(distance(x, y, a, b) <= 2.5 for a, b in zip(POINTS, POINTS[1:])):
        color = ACCENT
    if any(math.hypot(x - px, y - py) <= 5 for px, py in POINTS):
        color = ACCENT
    return color


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


def main():
    rows = bytearray()
    for y in range(SIZE):
        rows.append(0)
        for x in range(SIZE):
            samples = [sample(x + (sx + .5) / SCALE, y + (sy + .5) / SCALE) for sy in range(SCALE) for sx in range(SCALE)]
            for channel in range(4):
                rows.append(round(sum(pixel[channel] for pixel in samples) / len(samples)))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b'IDAT', zlib.compress(bytes(rows), 9)) + chunk(b'IEND', b'')
    (HERE / 'solvercraft.png').write_bytes(png)
    (HERE / 'solvercraft.svg').write_text('''<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <rect x="4" y="4" width="120" height="120" rx="12" fill="#0f1f2d"/>
  <path d="M27 29V100H103" fill="none" stroke="#84a6b7" stroke-width="2.5"/>
  <path d="M28 85L49 63L72 73L100 35" fill="none" stroke="#43d4c2" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
  <g fill="#43d4c2"><circle cx="28" cy="85" r="5"/><circle cx="49" cy="63" r="5"/><circle cx="72" cy="73" r="5"/><circle cx="100" cy="35" r="5"/></g>
</svg>
''', encoding='utf8', newline='\n')


if __name__ == '__main__':
    main()
