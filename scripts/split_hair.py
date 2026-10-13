# Splits a character's "Hair" primitive into five primitives with materials Hair_1..Hair_5 that the
# client hides stage by stage as HP drops: a bald spot on the crown, then receding temples, a
# thinning top, only the ring around the head, and finally a completely bald head.
# Usage: python3 -I scripts/split_hair.py in.glb out.glb
import json, struct, sys

src, dst = sys.argv[1], sys.argv[2]
b = open(src, 'rb').read()
jl = struct.unpack('<I', b[12:16])[0]
j = json.loads(b[20:20 + jl])
bs = 20 + jl
binlen = struct.unpack('<I', b[bs:bs + 4])[0]
bin_ = bytearray(b[bs + 8:bs + 8 + binlen])
mats = [m['name'] for m in j['materials']]
prims = j['meshes'][0]['primitives']
hi = next(i for i, p in enumerate(prims) if mats[p['material']] == 'Hair')
hair = prims[hi]


def acc(i):
    a = j['accessors'][i]
    v = j['bufferViews'][a['bufferView']]
    return a, v.get('byteOffset', 0) + a.get('byteOffset', 0), v.get('byteStride', 0)


pa, po, ps = acc(hair['attributes']['POSITION'])
ps = ps or 12
pos = [struct.unpack_from('<3f', bin_, po + k * ps) for k in range(pa['count'])]
ia, io, _ = acc(hair['indices'])
idx = struct.unpack_from('<%dH' % ia['count'], bin_, io)
tris = [idx[t:t + 3] for t in range(0, len(idx), 3)]


top_y = max(p[1] for p in pos)
low_y = min(p[1] for p in pos)
back = min(p[2] for p in pos)
CROWN = (0.0, top_y - 0.08, back * 0.45)
# Below this height the hair forms the ring around the head that survives the longest.
RING_Y = low_y + 0.45 * (top_y - low_y)


def centre(tri):
    return [sum(pos[i][k] for i in tri) / 3 for k in range(3)]


def dist(c):
    return sum((c[k] - CROWN[k]) ** 2 for k in range(3))


def thin_cell(c):
    # Checker pattern on |x| so both halves of the head thin the same way.
    cell = 0.11
    return (int(abs(c[0]) / cell) + int((c[2] + 1.0) / cell) + int(c[1] / cell)) % 2 == 0


groups = [[] for _ in range(5)]
top = []
for tri in tris:
    c = centre(tri)
    if c[1] < RING_Y:
        groups[4].append(tri)  # stage 5: the ring
    else:
        top.append((dist(c), c, tri))
top.sort(key=lambda t: t[0])
spot = int(len(top) * 0.12)
for rank, (_, c, tri) in enumerate(top):
    temple = c[2] > 0.22 and 0.16 < abs(c[0]) < 0.55
    if rank < spot:
        groups[0].append(tri)  # stage 1: bald spot on the crown
    elif rank < spot * 2 or temple:
        groups[1].append(tri)  # stage 2: bigger spot and receding temples
    elif thin_cell(c):
        groups[2].append(tri)  # stage 3: thinning top
    else:
        groups[3].append(tri)  # stage 4: rest of the top, leaving only the ring
base_mat = j['materials'][hair['material']]
new_prims = []
for stage in range(5):
    group = [i for tri in groups[stage] for i in tri]
    while len(bin_) % 4:
        bin_.append(0)
    off = len(bin_)
    bin_ += struct.pack('<%dH' % len(group), *group)
    j['bufferViews'].append({'buffer': 0, 'byteOffset': off, 'byteLength': 2 * len(group), 'target': 34963})
    j['accessors'].append({'bufferView': len(j['bufferViews']) - 1, 'componentType': 5123,
                           'count': len(group), 'type': 'SCALAR'})
    mat = dict(base_mat, name='Hair_%d' % (stage + 1))
    j['materials'].append(mat)
    p = dict(hair, indices=len(j['accessors']) - 1, material=len(j['materials']) - 1)
    new_prims.append(p)
prims[hi:hi + 1] = new_prims
while len(bin_) % 4:
    bin_.append(0)
j['buffers'][0]['byteLength'] = len(bin_)
js = json.dumps(j, separators=(',', ':')).encode()
js += b' ' * ((4 - len(js) % 4) % 4)
out = (b'glTF' + struct.pack('<II', 2, 12 + 8 + len(js) + 8 + len(bin_)) + struct.pack('<I', len(js))
       + b'JSON' + js + struct.pack('<I', len(bin_)) + b'BIN\0' + bin_)
open(dst, 'wb').write(out)
print('stages', [len(g) for g in groups], 'bytes', len(out))
