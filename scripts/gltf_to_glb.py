"""Pack a .gltf with embedded base64 buffers into a single .glb.

Usage: python3 -I scripts/gltf_to_glb.py in.gltf out.glb

Bevy loads a .glb on wasm without extra file requests; the Quaternius characters
ship as .gltf with data: URIs and use material colours only (no images).
"""
import base64
import json
import struct
import sys


def main(src, dst):
    with open(src, encoding="utf-8") as f:
        doc = json.load(f)
    if doc.get("images"):
        sys.exit("images present: external textures are not handled")
    blob = bytearray()
    offsets = []
    for buf in doc["buffers"]:
        uri = buf["uri"]
        if not uri.startswith("data:"):
            sys.exit("external buffer: " + uri)
        data = base64.b64decode(uri.split(",", 1)[1])
        while len(blob) % 4:
            blob.append(0)
        offsets.append(len(blob))
        blob += data
    for view in doc["bufferViews"]:
        view["byteOffset"] = offsets[view["buffer"]] + view.get("byteOffset", 0)
        view["buffer"] = 0
    doc["buffers"] = [{"byteLength": len(blob)}]
    js = json.dumps(doc, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    blob += b"\0" * (-len(blob) % 4)
    total = 12 + 8 + len(js) + 8 + len(blob)
    with open(dst, "wb") as f:
        f.write(struct.pack("<4sII", b"glTF", 2, total))
        f.write(struct.pack("<I4s", len(js), b"JSON") + js)
        f.write(struct.pack("<I4s", len(blob), b"BIN\0") + bytes(blob))


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
