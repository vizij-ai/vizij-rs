"""Prints a wasm file's sections by size (custom sections by name)."""
import sys
d = open(sys.argv[1], 'rb').read()
def leb(i):
    r = 0; s = 0
    while True:
        b = d[i]; i += 1; r |= (b & 0x7f) << s; s += 7
        if not b & 0x80: return r, i
names = {0:'custom',1:'type',2:'import',3:'function',4:'table',5:'memory',6:'global',7:'export',8:'start',9:'element',10:'code',11:'data',12:'datacount'}
i = 8; secs = {}
while i < len(d):
    sid = d[i]; i += 1
    size, i = leb(i)
    label = names.get(sid, str(sid))
    if sid == 0:
        nlen, j = leb(i); label = 'custom:' + d[j:j+nlen].decode('utf8', 'replace')
    secs[label] = secs.get(label, 0) + size
    i += size
for k, v in sorted(secs.items(), key=lambda kv: -kv[1]): print(f"{k:24s} {v:>12,d}")
print('total', len(d))
