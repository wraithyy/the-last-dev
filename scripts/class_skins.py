# Builds the Frontend / Backend / DevOps player models from the Quaternius "Ultimate Animated
# Character Pack" (CC0). Every class keeps the Casual2_Male skeleton, proportions and animations
# (same as Engineer.glb); only outfit, hair and a few extra skinned bits differ.
# The QA Engineer, Data Scientist, Security Engineer and COBOL Greybeard are reskins of those (RESKINS): same mesh, hood and
# hair stages, new colours and clothing texture, so they need only the built GLBs.
# Usage: python3 -I scripts/class_skins.py "<pack>/glTF" assets/models/characters
#        python3 -I scripts/class_skins.py --reskin assets/models/characters
import base64
import json
import math
import os
import struct
import subprocess
import sys
import tempfile
import zlib

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
BODY = "Casual2_Male"  # same body as Engineer.glb, so rig, scale and gear bones match
ARM_JOINTS = ("UpperArm.L", "LowerArm.L", "UpperArm.R", "LowerArm.R")
TILE = 0.28  # world size of one texture repeat on the clothes

CLASSES = {
    "Frontend": dict(hair_from="Casual2_Female", hair=(0.0, 0.32, 0.30), skin=(0.80, 0.52, 0.38),
                     shirt=None, sleeves=(0.0, 0.30, 0.28), hood=(0.0, 0.30, 0.28),
                     pants=(0.06, 0.03, 0.22), belt=(0.85, 0.55, 0.0), bags=None, thick=0.0),
    "Backend": dict(hair_from="Casual3_Female", hair=(0.045, 0.035, 0.03), skin=(0.70, 0.48, 0.38),
                    shirt=(0.028, 0.028, 0.034), sleeves=(0.028, 0.028, 0.034), hood=(0.022, 0.022, 0.028),
                    pants=(0.010, 0.010, 0.014), belt=(0.01, 0.01, 0.01), bags=(0.10, 0.045, 0.07), thick=0.2),
    "DevOps": dict(hair_from="Casual_Male", hair=(0.20, 0.08, 0.025), skin=(0.78, 0.50, 0.34),
                   shirt=None, sleeves=None, hood=None,
                   pants=(0.05, 0.12, 0.30), belt=(0.06, 0.03, 0.015), bags=(0.16, 0.07, 0.07), thick=0.0),
}


# name -> (built model it reuses, material colours by name, clothing texture or None)
RESKINS = {
    "QA": ("Frontend", {"Skin": (0.62, 0.42, 0.31), "Hair": (0.36, 0.10, 0.04), "Pants": (0.17, 0.17, 0.19),
                        "Belt": (0.70, 0.05, 0.05), "Sleeves": (0.03, 0.03, 0.035), "Hood": (0.03, 0.03, 0.035)},
           "caution"),
    "DataScientist": ("DevOps", {"Skin": (0.55, 0.36, 0.25), "Hair": (0.02, 0.02, 0.025),
                                 "Pants": (0.42, 0.36, 0.24), "Belt": (0.12, 0.07, 0.03)},
                      "graph_paper"),
    "SecurityEngineer": ("Backend", {"Skin": (0.78, 0.60, 0.50), "Hair": (0.50, 0.52, 0.58),
                                     "Pants": (0.015, 0.015, 0.02), "Belt": (0.02, 0.02, 0.02),
                                     "Sleeves": (0.012, 0.012, 0.015), "Hood": (0.012, 0.012, 0.015)},
                         None),
    "CobolGreybeard": ("DevOps", {"Skin": (0.76, 0.57, 0.47), "Hair": (0.66, 0.66, 0.69),
                                  "Pants": (0.2, 0.18, 0.15), "Belt": (0.05, 0.03, 0.02)},
                       "cardigan"),
}


def load_gltf(pack, name):
    with open(os.path.join(pack, name + ".gltf"), encoding="utf-8") as f:
        doc = json.load(f)
    return doc, base64.b64decode(doc["buffers"][0]["uri"].split(",", 1)[1])


def read(doc, blob, idx):
    a = doc["accessors"][idx]
    v = doc["bufferViews"][a["bufferView"]]
    n = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[a["type"]]
    dt = {5126: "<f4", 5123: "<u2", 5125: "<u4", 5121: "u1"}[a["componentType"]]
    off = v.get("byteOffset", 0) + a.get("byteOffset", 0)
    return np.frombuffer(blob, dtype=dt, count=a["count"] * n, offset=off).reshape(a["count"], n).copy()


def prim_arrays(doc, blob, prim):
    at = prim["attributes"]
    return dict(pos=read(doc, blob, at["POSITION"]).astype(float), nor=read(doc, blob, at["NORMAL"]).astype(float),
                joints=read(doc, blob, at["JOINTS_0"]), weights=read(doc, blob, at["WEIGHTS_0"]).astype(float),
                idx=read(doc, blob, prim["indices"]).reshape(-1, 3).astype(int))


def subset(arr, tri_mask):
    # Re-index only the vertices the selected triangles use.
    tris = arr["idx"][tri_mask]
    used, inv = np.unique(tris, return_inverse=True)
    out = {k: arr[k][used] for k in ("pos", "nor", "joints", "weights")}
    out["idx"] = inv.reshape(-1, 3)
    return out


class Writer:
    def __init__(self, doc, blob):
        self.doc, self.blob = doc, bytearray(blob)

    def _view(self, data, target):
        while len(self.blob) % 4:
            self.blob.append(0)
        self.doc["bufferViews"].append({"buffer": 0, "byteOffset": len(self.blob), "byteLength": len(data),
                                        **({"target": target} if target else {})})
        self.blob += data
        return len(self.doc["bufferViews"]) - 1

    def acc(self, arr, ctype, typ, target=34962, minmax=False):
        dt = {5126: "<f4", 5123: "<u2", 5121: "u1"}[ctype]
        arr = np.ascontiguousarray(arr, dtype=dt)
        a = {"bufferView": self._view(arr.tobytes(), target), "componentType": ctype, "count": len(arr), "type": typ}
        if minmax:
            a["min"], a["max"] = arr.min(0).tolist(), arr.max(0).tolist()
        self.doc["accessors"].append(a)
        return len(self.doc["accessors"]) - 1

    def prim(self, arr, material, uv=None):
        at = {"POSITION": self.acc(arr["pos"], 5126, "VEC3", minmax=True), "NORMAL": self.acc(arr["nor"], 5126, "VEC3"),
              "JOINTS_0": self.acc(arr["joints"], 5121, "VEC4"), "WEIGHTS_0": self.acc(arr["weights"], 5126, "VEC4")}
        if uv is not None:
            at["TEXCOORD_0"] = self.acc(uv, 5126, "VEC2")
        return {"attributes": at, "indices": self.acc(arr["idx"].reshape(-1), 5123, "SCALAR", 34963),
                "material": material, "mode": 4}


def unweld(arr, uv_fn):
    # One vertex per triangle corner so a texture seam can wrap without smearing a whole triangle.
    flat = arr["idx"].reshape(-1)
    out = {k: arr[k][flat] for k in ("pos", "nor", "joints", "weights")}
    out["idx"] = np.arange(len(flat)).reshape(-1, 3)
    uv = uv_fn(out["pos"]).reshape(-1, 3, 2)
    for t in range(len(uv)):
        u = uv[t, :, 0]
        if u.max() - u.min() > math.pi * 0.45 / TILE:  # crossed the +-pi seam
            uv[t, u < u.mean(), 0] += 2 * math.pi * 0.45 / TILE
    return out, uv.reshape(-1, 2)


def torso_uv(p):
    return np.stack([np.arctan2(p[:, 0], p[:, 2] + 0.05) * 0.45 / TILE, -p[:, 1] / TILE], 1)


def band_uv(p):
    # Single-column texture stretched over the shirt height; u is irrelevant.
    return np.stack([np.zeros(len(p)), 1 - (p[:, 1] - 1.13) / 0.93], 1)


def sleeve_uv(p):
    return np.stack([np.arctan2(p[:, 1] - 1.88, p[:, 2]) * 0.13 / TILE, np.abs(p[:, 0]) / TILE], 1)


def png(pixels):
    h, w = len(pixels), len(pixels[0])
    raw = b"".join(b"\0" + bytes(c for px in row for c in px) for row in pixels)

    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d))
    # 8-bit RGB: 16-bit PNGs crash WebGL2 texture upload.
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) \
        + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def flannel_png():
    base, dark, line = (150, 34, 30), (66, 18, 20), (214, 168, 120)
    px = [[None] * 16 for _ in range(16)]
    for y in range(16):
        for x in range(16):
            c = base
            if 2 <= x <= 6:
                c = tuple(int(v * 0.62) for v in c)
            if 2 <= y <= 6:
                c = tuple(int(v * 0.62) for v in c)
            if 2 <= x <= 6 and 2 <= y <= 6:
                c = dark
            if x == 11 or y == 11:
                c = tuple((a + b * 2) // 3 for a, b in zip(c, line))
            px[y][x] = c
    return png(px)


def stripes_png():
    # Magenta hoodie with a teal+yellow chest band and a teal hem, as one 2x32 column.
    rows = []
    for y in range(32):
        c = (226, 30, 150)
        if 6 <= y <= 8:
            c = (20, 190, 180)
        elif 9 <= y <= 11:
            c = (250, 215, 30)
        elif y >= 29:
            c = (20, 190, 180)
        rows.append([c, c])
    return png(rows)


def caution_png():
    # Hi-vis yellow hoodie with a black/yellow "caution: untested" chest band, as one 2x32 column.
    rows = []
    for y in range(32):
        c = (236, 196, 18)
        if 6 <= y <= 7 or 10 <= y <= 11 or y >= 29:
            c = (22, 22, 24)
        rows.append([c, c])
    return png(rows)


def graph_paper_png():
    # Purple sweater knitted like graph paper, with one rising "trend line" pixel per column.
    base, grid, trend = (84, 38, 132), (146, 104, 196), (120, 230, 170)
    px = [[base] * 16 for _ in range(16)]
    for y in range(16):
        for x in range(16):
            if x % 4 == 0 or y % 4 == 0:
                px[y][x] = grid
    for x in range(16):
        px[15 - x][x] = trend
    return png(px)


def cardigan_png():
    # Brown knit cardigan: vertical ribs and a darker purl row, wrapped around the torso and arms.
    base, rib, purl = (116, 86, 58), (88, 64, 42), (100, 74, 50)
    px = [[base] * 16 for _ in range(16)]
    for y in range(16):
        for x in range(16):
            if x % 4 == 0:
                px[y][x] = rib
            elif y % 8 == 7:
                px[y][x] = purl
    return png(px)


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    n = struct.unpack("<I", data[12:16])[0]
    doc = json.loads(data[20:20 + n])
    return doc, data[20 + n + 8:]


def reskin(outdir, name, src, colours, texture):
    doc, blob = read_glb(os.path.join(outdir, src + ".glb"))
    for m in doc["materials"]:
        base = m["name"].split("_")[0]  # Hair_1..Hair_5 follow Hair
        if base in colours:
            m["pbrMetallicRoughness"]["baseColorFactor"] = list(colours[base]) + [1.0]
    w = Writer(doc, blob)
    if texture:
        data = {"caution": caution_png, "graph_paper": graph_paper_png, "cardigan": cardigan_png}[texture]()
        doc["images"][0]["bufferView"] = w._view(data, None)
    write_glb(doc, compact(doc, bytes(w.blob)), os.path.join(outdir, name + ".glb"))


def add_texture(w, data):
    w.doc.setdefault("images", []).append({"bufferView": w._view(data, None), "mimeType": "image/png"})
    w.doc.setdefault("samplers", [{"magFilter": 9728, "minFilter": 9728, "wrapS": 10497, "wrapT": 10497}])
    w.doc.setdefault("textures", []).append({"source": len(w.doc["images"]) - 1, "sampler": 0})
    return len(w.doc["textures"]) - 1


def add_material(doc, name, color, like, tex=None):
    m = json.loads(json.dumps(doc["materials"][like]))
    m["name"] = name
    m["pbrMetallicRoughness"]["baseColorFactor"] = list(color) + [1.0]
    m["pbrMetallicRoughness"].pop("baseColorTexture", None)  # `like` may already be textured
    if tex is not None:
        apply_texture(m, tex)
    doc["materials"].append(m)
    return len(doc["materials"]) - 1


def apply_texture(m, tex):
    m["pbrMetallicRoughness"]["baseColorFactor"] = [1, 1, 1, 1]
    m["pbrMetallicRoughness"]["baseColorTexture"] = {"index": tex}


def smooth(t):
    t = np.clip(t, 0, 1)
    return t * t * (3 - 2 * t)


def thicken(arr, amount):
    # Widen only the torso vertices; the skeleton stays untouched so animations still fit.
    p = arr["pos"]
    w = smooth((p[:, 1] - 1.08) / 0.25) * (1 - smooth((p[:, 1] - 1.75) / 0.3)) * (1 - smooth((np.abs(p[:, 0]) - 0.35) / 0.2))
    f = 1 + amount * w
    p[:, 0] *= f
    p[:, 2] = (p[:, 2] + 0.05) * f - 0.05


def hood(joint):
    # Flattened ellipsoid bunched behind the neck, rigid to the Neck bone.
    c, r = np.array([0.0, 2.12, -0.13]), np.array([0.36, 0.13, 0.19])
    n_lat, n_lon = 8, 14
    pts, idx = [], []
    for i in range(n_lat + 1):
        th = math.pi * i / n_lat
        for k in range(n_lon):
            ph = 2 * math.pi * k / n_lon
            pts.append([math.sin(th) * math.cos(ph), math.cos(th), math.sin(th) * math.sin(ph)])
    for i in range(n_lat):
        for k in range(n_lon):
            a, b = i * n_lon + k, i * n_lon + (k + 1) % n_lon
            idx += [[a, a + n_lon, b], [b, a + n_lon, b + n_lon]]
    u = np.array(pts)
    nor = u / r
    nor /= np.linalg.norm(nor, axis=1)[:, None]
    return rig(c + u * r, nor, np.array(idx), joint)


def rig(pos, nor, idx, joint):
    n = len(pos)
    j = np.zeros((n, 4), int)
    j[:, 0] = joint
    wt = np.zeros((n, 4))
    wt[:, 0] = 1
    return dict(pos=pos, nor=nor, joints=j, weights=wt, idx=idx)


def eye_bags(face, head_tris, head_joint):
    # Dark ellipses just under each eye, laid on the face surface found by a ray cast along -z.
    P, T = head_tris
    v0, e1, e2 = P[T[:, 0]], P[T[:, 1]] - P[T[:, 0]], P[T[:, 2]] - P[T[:, 0]]

    def surface_z(x, y):
        # Moller-Trumbore for a ray (x, y, 5) -> (0, 0, -1); keep the highest hit.
        d = np.array([0.0, 0.0, -1.0])
        pv = np.cross(d, e2)
        det = (e1 * pv).sum(1)
        ok = np.abs(det) > 1e-9
        inv = np.where(ok, 1 / np.where(ok, det, 1), 0)
        tv = np.array([x, y, 5.0]) - v0
        u = (tv * pv).sum(1) * inv
        qv = np.cross(tv, e1)
        v = (qv * d).sum(1) * inv
        t = (e2 * qv).sum(1) * inv
        hit = ok & (u >= 0) & (v >= 0) & (u + v <= 1)
        return 5 - t[hit].min() if hit.any() else None

    eyes = face[face[:, 1] < 2.66]  # eyes only, brows start above
    pos, idx = [], []
    for s in (-1, 1):
        e = eyes[np.sign(eyes[:, 0]) == s]
        cx, cy = e[:, 0].mean(), e[:, 1].min() - 0.022
        base = len(pos)
        pos.append([cx, cy, surface_z(cx, cy)])
        for ring in (0.5, 1.0):
            for k in range(12):
                a = 2 * math.pi * k / 12
                x, y = cx + 0.075 * ring * math.cos(a), cy + 0.032 * ring * math.sin(a)
                pos.append([x, y, surface_z(x, y)])
        for k in range(12):
            a0, a1 = 1 + k, 1 + (k + 1) % 12
            b0, b1 = 13 + k, 13 + (k + 1) % 12
            idx += [[base, base + a0, base + a1], [base + a0, base + b0, base + b1], [base + a0, base + b1, base + a1]]
    pos = np.array(pos, float)
    pos[:, 2] += 0.012  # lift off the skin so it does not z-fight
    nor = np.tile([0.0, 0.0, 1.0], (len(pos), 1))
    return rig(pos, nor, np.array(idx), head_joint)


def build(pack, name, cfg, tmp):
    doc, blob = load_gltf(pack, BODY)
    hdoc, hblob = load_gltf(pack, cfg["hair_from"])
    jn = [doc["nodes"][i]["name"] for i in doc["skins"][0]["joints"]]
    # Merging vertex data is only valid when both skins share joint order and bind poses.
    assert jn == [hdoc["nodes"][i]["name"] for i in hdoc["skins"][0]["joints"]]
    assert read(doc, blob, doc["skins"][0]["inverseBindMatrices"]).tobytes() == \
        read(hdoc, hblob, hdoc["skins"][0]["inverseBindMatrices"]).tobytes()
    mats = [m["name"] for m in doc["materials"]]
    prims = {mats[p["material"]]: prim_arrays(doc, blob, p) for p in doc["meshes"][0]["primitives"]}
    hprims = {m["name"]: p for m, p in zip([hdoc["materials"][p["material"]] for p in hdoc["meshes"][0]["primitives"]],
                                           hdoc["meshes"][0]["primitives"])}
    hair = prim_arrays(hdoc, hblob, hprims["Hair"])
    face = prims["Face"]["pos"]
    skin = prims["Skin"]

    for m in doc["materials"]:
        col = {"Skin": cfg["skin"], "Pants": cfg["pants"], "Belt": cfg["belt"], "Hair": cfg["hair"],
               "Face": (0, 0, 0)}.get(m["name"])
        if col:
            m["pbrMetallicRoughness"]["baseColorFactor"] = list(col) + [1.0]
    shirt_i = mats.index("Shirt")
    w = Writer(doc, blob)
    out = []  # (prim dict, material index)
    mi = {n: mats.index(n) for n in mats}

    dom = skin["joints"][np.arange(len(skin["joints"])), skin["weights"].argmax(1)]
    arm = np.isin(dom, [jn.index(j) for j in ARM_JOINTS])
    arm_tri = arm[skin["idx"]].all(1)
    sleeves = subset(skin, arm_tri)
    skin_rest = subset(skin, ~arm_tri)

    shirt = prims["Shirt"]
    if cfg["thick"]:
        thicken(shirt, cfg["thick"])
        thicken(prims["Belt"], cfg["thick"])
    flannel = name == "DevOps"
    tex = None
    if flannel or name == "Frontend":
        tex = add_texture(w, flannel_png() if flannel else stripes_png())
    if name == "Frontend":
        shirt_arr, shirt_uv = unweld(shirt, band_uv)
        apply_texture(doc["materials"][shirt_i], tex)
    elif flannel:
        shirt_arr, shirt_uv = unweld(shirt, torso_uv)
        apply_texture(doc["materials"][shirt_i], tex)
    else:
        shirt_arr, shirt_uv = shirt, None
        doc["materials"][shirt_i]["pbrMetallicRoughness"]["baseColorFactor"] = list(cfg["shirt"]) + [1.0]

    out.append((skin_rest, mi["Skin"], None))
    if flannel:
        sl_arr, sl_uv = unweld(sleeves, sleeve_uv)
        out.append((sl_arr, add_material(doc, "Sleeves", (1, 1, 1), shirt_i, tex), sl_uv))
    else:
        out.append((sleeves, add_material(doc, "Sleeves", cfg["sleeves"], shirt_i), None))
    out += [(shirt_arr, shirt_i, shirt_uv), (prims["Pants"], mi["Pants"], None), (prims["Belt"], mi["Belt"], None),
            (prims["Face"], mi["Face"], None)]
    if cfg["hood"]:
        out.append((hood(jn.index("Neck")), add_material(doc, "Hood", cfg["hood"], shirt_i), None))
    if cfg["bags"]:
        head = dom == jn.index("Head")
        head_tri = head[skin["idx"]].all(1)
        tris = np.concatenate([skin["pos"][skin["idx"][head_tri]], prims["Face"]["pos"][prims["Face"]["idx"]]])
        P = tris.reshape(-1, 3)
        out.append((eye_bags(face, (P, np.arange(len(P)).reshape(-1, 3)), jn.index("Head")),
                    add_material(doc, "Shadow", cfg["bags"], shirt_i), None))
    out.append((hair, mi["Hair"], None))
    for m in doc["materials"]:
        m["doubleSided"] = True if m["name"] in ("Hood", "Shadow") else m.get("doubleSided", False)

    doc["meshes"][0]["primitives"] = [w.prim(a, m, uv) for a, m, uv in out]
    doc["buffers"] = [{"byteLength": len(w.blob)}]
    path = os.path.join(tmp, name + "_pre.glb")
    write_glb(doc, compact(doc, bytes(w.blob)), path)
    return path


def compact(doc, blob):
    # Drop accessors/views orphaned by the replaced primitives (the donor body's own mesh data).
    used = set()
    for m in doc["meshes"]:
        for p in m["primitives"]:
            used |= set(p["attributes"].values()) | {p["indices"]}
    for a in doc.get("animations", []):
        used |= {s["input"] for s in a["samplers"]} | {s["output"] for s in a["samplers"]}
    used |= {sk["inverseBindMatrices"] for sk in doc["skins"] if "inverseBindMatrices" in sk}
    views = {doc["accessors"][i]["bufferView"] for i in used} | {i["bufferView"] for i in doc.get("images", [])}
    vmap, amap, nb, nv, na = {}, {}, bytearray(), [], []
    for i in sorted(views):
        v = dict(doc["bufferViews"][i])
        nb += b"\0" * (-len(nb) % 4)
        start = v.get("byteOffset", 0)
        data = blob[start:start + v["byteLength"]]
        v["byteOffset"] = len(nb)
        nb += data
        vmap[i] = len(nv)
        nv.append(v)
    for i in sorted(used):
        a = dict(doc["accessors"][i])
        a["bufferView"] = vmap[a["bufferView"]]
        amap[i] = len(na)
        na.append(a)
    for m in doc["meshes"]:
        for p in m["primitives"]:
            p["attributes"] = {k: amap[v] for k, v in p["attributes"].items()}
            p["indices"] = amap[p["indices"]]
    for a in doc.get("animations", []):
        for s in a["samplers"]:
            s["input"], s["output"] = amap[s["input"]], amap[s["output"]]
    for sk in doc["skins"]:
        if "inverseBindMatrices" in sk:
            sk["inverseBindMatrices"] = amap[sk["inverseBindMatrices"]]
    for i in doc.get("images", []):
        i["bufferView"] = vmap[i["bufferView"]]
    doc["bufferViews"], doc["accessors"] = nv, na
    return bytes(nb)


def write_glb(doc, blob, path):
    blob += b"\0" * (-len(blob) % 4)
    doc["buffers"] = [{"byteLength": len(blob)}]
    js = json.dumps(doc, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    with open(path, "wb") as f:
        f.write(struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(js) + 8 + len(blob)))
        f.write(struct.pack("<I4s", len(js), b"JSON") + js + struct.pack("<I4s", len(blob), b"BIN\0") + blob)


def reskin_all(outdir):
    for name, (src, colours, texture) in RESKINS.items():
        reskin(outdir, name, src, colours, texture)


def main(pack, outdir):
    with tempfile.TemporaryDirectory() as tmp:
        for name, cfg in CLASSES.items():
            pre = build(pack, name, cfg, tmp)
            # Hair staging lives in split_hair.py so all characters bald the same way.
            subprocess.run([sys.executable, "-I", os.path.join(HERE, "split_hair.py"), pre,
                            os.path.join(outdir, name + ".glb")], check=True)
    reskin_all(outdir)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: python3 -I scripts/class_skins.py (<pack>/glTF | --reskin) assets/models/characters")
    if sys.argv[1] == "--reskin":
        reskin_all(sys.argv[2])
    else:
        main(sys.argv[1], sys.argv[2])
