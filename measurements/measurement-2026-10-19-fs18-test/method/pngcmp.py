# pngcmp.py <dir> <tagA> <tagB> -- pixel compare of screendumps <tagA>-X.png vs <tagB>-X.png (PIL): identical / differing pixel count + bbox
import sys, glob, os
from PIL import Image, ImageChops
d, a, b = sys.argv[1:4]
for fa in sorted(glob.glob(os.path.join(d, a + '-*.png'))):
    name = os.path.basename(fa)[len(a) + 1:]; fb = os.path.join(d, b + '-' + name)
    if not os.path.exists(fb): print(f'{name}: MISSING in {b}'); continue
    A = Image.open(fa).convert('RGB'); B = Image.open(fb).convert('RGB')
    if A.size != B.size: print(f'{name}: size {A.size} vs {B.size}'); continue
    diff = ImageChops.difference(A, B); bb = diff.getbbox()
    if bb is None: print(f'{name}: IDENTICAL'); continue
    n = sum(1 for p in diff.getdata() if p != (0, 0, 0))
    print(f'{name}: DIFF {n} px bbox={bb}')
