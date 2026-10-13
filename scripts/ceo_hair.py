# Derives the CEO head from Suit_Male.glb: short hair hugging the scalp, a receding hairline
# (high forehead) and a grey-to-black vertex colour gradient from the sides to the top.
# Usage: python3 -I scripts/ceo_hair.py assets/models/characters/Suit_Male.glb \
#   assets/models/characters/Suit_Male_Bald.glb cap=0.055 fz=0.22 fy=2.95 y0=2.55 y1=2.82 grey=0.55 top=0.07 thin=3
import json,struct,sys,math
src,dst=sys.argv[1],sys.argv[2]
P=dict(a.split('=') for a in sys.argv[3:])
CAP=float(P.get('cap',0.035)); FRONT_Z=float(P.get('fz',0.22)); FRONT_Y=float(P.get('fy',2.78))
GREY=float(P.get('grey',0.62)); TOP=float(P.get('top',0.02)); THIN=int(P.get('thin',0)); THIN_Y=float(P.get('thin_y',2.85)); Y0=float(P.get('y0',2.55)); Y1=float(P.get('y1',2.95))
b=open(src,'rb').read()
jl=struct.unpack('<I',b[12:16])[0]; j=json.loads(b[20:20+jl])
bs=20+jl; binlen=struct.unpack('<I',b[bs:bs+4])[0]; bin_=bytearray(b[bs+8:bs+8+binlen])
mats=[m['name'] for m in j['materials']]
prims=j['meshes'][0]['primitives']
def acc(i):
    a=j['accessors'][i]; v=j['bufferViews'][a['bufferView']]
    return a,v,v.get('byteOffset',0)+a.get('byteOffset',0),v.get('byteStride',0)
def readv(i,n=3):
    a,v,o,s=acc(i); s=s or 4*n
    return [list(struct.unpack_from('<%df'%n,bin_,o+k*s)) for k in range(a['count'])],o,s
hair=next(p for p in prims if mats[p['material']]=='Hair')
skin=next(p for p in prims if mats[p['material']]=='Skin')
sk,_,_=readv(skin['attributes']['POSITION']); head=[q for q in sk if q[1]>2.3]
hp,ho,hs=readv(hair['attributes']['POSITION'])
for q in hp:
    n=min(head,key=lambda h:(h[0]-q[0])**2+(h[1]-q[1])**2+(h[2]-q[2])**2)
    d=[q[k]-n[k] for k in range(3)]; L=math.sqrt(sum(x*x for x in d)) or 1e-6
    if L>CAP:
        for k in range(3): q[k]=n[k]+d[k]/L*CAP
for k,q in enumerate(hp): struct.pack_into('<3f',bin_,ho+k*hs,*q)
a=j['accessors'][hair['attributes']['POSITION']]
a['min']=[min(q[i] for q in hp) for i in range(3)]; a['max']=[max(q[i] for q in hp) for i in range(3)]
ia,iv,io,_=acc(hair['indices']); idx=list(struct.unpack_from('<%dH'%ia['count'],bin_,io))
keep=[]
for t in range(0,len(idx),3):
    tri=idx[t:t+3]; cy=sum(hp[i][1] for i in tri)/3; cz=sum(hp[i][2] for i in tri)/3
    # Thinning crown: drop every THIN-th triangle on top of the head.
    if THIN and cy>THIN_Y and (t//3)%THIN==0: continue
    if not (cz>FRONT_Z and cy>FRONT_Y-0.25 and cy<FRONT_Y+0.45 and abs(sum(hp[i][0] for i in tri)/3)<0.42): keep+=tri
print('dropped',(len(idx)-len(keep))//3,'of',len(idx)//3)
keep+=[0]*(len(idx)-len(keep)); struct.pack_into('<%dH'%len(keep),bin_,io,*keep)
cols=[]
for q in hp:
    t=min(1,max(0,(q[1]-Y0)/(Y1-Y0))); c=GREY*(1-t)+TOP*t; cols.append((c,c,c*1.02,1.0))
while len(bin_)%4: bin_.append(0)
off=len(bin_)
for c in cols: bin_+=struct.pack('<4f',*c)
j['bufferViews'].append({'buffer':0,'byteOffset':off,'byteLength':16*len(cols)})
j['accessors'].append({'bufferView':len(j['bufferViews'])-1,'componentType':5126,'count':len(cols),'type':'VEC4'})
hair['attributes']['COLOR_0']=len(j['accessors'])-1
j['materials'][hair['material']]['pbrMetallicRoughness']['baseColorFactor']=[1,1,1,1]
j['buffers'][0]['byteLength']=len(bin_)
js=json.dumps(j,separators=(',',':')).encode(); js+=b' '*((4-len(js)%4)%4)
out=b'glTF'+struct.pack('<II',2,12+8+len(js)+8+len(bin_))+struct.pack('<I',len(js))+b'JSON'+js+struct.pack('<I',len(bin_))+b'BIN\0'+bin_
open(dst,'wb').write(out); print('ok',len(out))
