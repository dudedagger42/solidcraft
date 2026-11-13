S=2
# full state for step S
H = 80 if S >= 3 else 60
k = sqrt(0.5)
with part('Inlet'):
    box(80,80,6,center=(0,0,3),name='Inlet'); box(54,34,6,center=(0,0,3),op='cut',into='Inlet')
    for x in (-32,32):
        for y in (-32,32): hole(at=(x,y,6),d=6)
with part('Duct'):
    a=sketch('XY',offset=6); a.rect_center((0,0),54,34); b=sketch('XY',offset=6+H); b.circle((0,0),d=44)
    loft([a,b],name='Duct')
    c=sketch('XY',offset=6); c.rect_center((0,0),50,30); e=sketch('XY',offset=6+H); e.circle((0,0),d=40)
    loft([c,e],op='cut',into='Duct')
top = 6+H+15
with part('Outlet'):
    cylinder(15,d=44,at=(0,0,6+H),name='Outlet'); cylinder(15,d=40,at=(0,0,6+H),op='cut',into='Outlet')
if S >= 1:
    with part('Elbow'):
        s=sketch('XY',offset=top); s.circle((0,0),d=44)
        cmd('solid.revolve',sketch=s,axis={'origin':[40,0,top],'dir':[0,-1,0]},angle='45 deg',operation='new',body_name='Elbow')
        s=sketch('XY',offset=top); s.circle((0,0),d=40)
        cmd('solid.revolve',sketch=s,axis={'origin':[40,0,top],'dir':[0,-1,0]},angle='45 deg',operation='cut',targets=['Elbow'])
ex, ez = 40-40*k, top+40*k
if S >= 2:
    with part('Pipe'):
        cmd('solid.cylinder',diameter=44,height=50,base=[ex,0,ez],axis=[k,0,k],operation='new',body_name='Pipe')
        cmd('solid.cylinder',diameter=40,height=50,base=[ex,0,ez],axis=[k,0,k],operation='cut',targets=['Pipe'])
if S >= 4:
    fx, fz = ex+44*k, ez+44*k
    with part('PipeFlange'):
        cmd('solid.cylinder',diameter=70,height=6,base=[fx,0,fz],axis=[k,0,k],operation='new',body_name='PipeFlange')
        cmd('solid.cylinder',diameter=44,height=6,base=[fx,0,fz],axis=[k,0,k],operation='cut',targets=['PipeFlange'])
