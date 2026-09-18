"""Changed-pixel clusters between two screenshots inside a crop box: count, and
the bounding box + centroid of each connected cluster of 8-px blocks."""
import sys
from collections import deque
from PIL import Image, ImageChops
a, b = Image.open(sys.argv[1]).convert('RGB'), Image.open(sys.argv[2]).convert('RGB')
box = tuple(int(v) for v in sys.argv[3].split(',')) if len(sys.argv) > 3 else (0, 0, a.size[0], a.size[1])
a, b = a.crop(box), b.crop(box)
diff = ImageChops.difference(a, b).convert('L')
w, h = diff.size
px = diff.load()
B = 8
bw, bh = (w + B - 1) // B, (h + B - 1) // B
blocks = [[0] * bw for _ in range(bh)]
changed = 0
for y in range(h):
    for x in range(w):
        if px[x, y] > 8:
            changed += 1
            blocks[y // B][x // B] += 1
seen = [[False] * bw for _ in range(bh)]
clusters = []
for by in range(bh):
    for bx in range(bw):
        if blocks[by][bx] == 0 or seen[by][bx]:
            continue
        q = deque([(by, bx)]); seen[by][bx] = True; cells = []
        while q:
            cy, cx = q.popleft(); cells.append((cy, cx))
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    ny, nx = cy + dy, cx + dx
                    if 0 <= ny < bh and 0 <= nx < bw and not seen[ny][nx] and blocks[ny][nx] > 0:
                        seen[ny][nx] = True; q.append((ny, nx))
        n = sum(blocks[cy][cx] for cy, cx in cells)
        xs = [cx for _, cx in cells]; ys = [cy for cy, _ in cells]
        clusters.append((n, (box[0] + min(xs) * B, box[1] + min(ys) * B, box[0] + (max(xs) + 1) * B, box[1] + (max(ys) + 1) * B),
                         (box[0] + (sum(xs) / len(xs) + 0.5) * B, box[1] + (sum(ys) / len(ys) + 0.5) * B)))
clusters.sort(reverse=True)
print(f"crop={box} size={a.size} changed_pixels={changed} ({100*changed/(w*h):.1f}%) clusters={len(clusters)}")
for n, bb, c in clusters[:6]:
    print(f"  cluster px={n} bbox={bb} centroid=({c[0]:.0f},{c[1]:.0f})")
