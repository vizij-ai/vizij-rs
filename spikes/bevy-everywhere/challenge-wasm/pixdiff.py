"""Compares two screenshots inside a crop box: distinct colors and changed pixels."""
import sys
from PIL import Image, ImageChops
a, b = Image.open(sys.argv[1]).convert('RGB'), Image.open(sys.argv[2]).convert('RGB')
box = tuple(int(v) for v in sys.argv[3].split(',')) if len(sys.argv) > 3 else None
if box:
    a, b = a.crop(box), b.crop(box)
diff = ImageChops.difference(a, b).convert('L')
changed = sum(1 for p in diff.getdata() if p > 8)
colors_a = len(set(a.getdata()))
print(f"crop={box} size={a.size} distinct_colors_1={colors_a} changed_pixels={changed} ({100*changed/(a.size[0]*a.size[1]):.1f}%)")
