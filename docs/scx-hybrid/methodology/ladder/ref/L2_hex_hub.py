s=sketch('XY'); s.polygon((0,0),6,40/sqrt(3)); s.extrude(30,name='HexHub')
hole(at=(0,0,30),d=24,depth=6); hole(at=(0,0,24),d=16)
for a in (30,90,150,210,270,330):
    c,sn=cos(radians(a)),sin(radians(a)); hole(at=(20*c,20*sn,15),d=6,depth=8)
m=(8+40/sqrt(3))/2
for x in (m,-m): hole(at=(x,0,0),d=5,depth=6)
