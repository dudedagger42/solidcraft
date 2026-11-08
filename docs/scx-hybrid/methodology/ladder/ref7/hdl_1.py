S=1
L = 140 if S >= 4 else 100
cx = L/2
box(L,100,8,name='Base')
if S >= 2:
    for x,y in [(10,10),(L-10,10),(10,90),(L-10,90)]:
        hole(at=(x,y,8),d=11,depth=4); hole(at=(x,y,4),d=6.6)
if S >= 5:
    fillet([(0,0,4),(L,0,4),(0,100,4),(L,100,4)],10)
def handle(tag):
    # path in the plane y=50 around x=cx: feet at cx-30 and cx+30
    x0, x1 = cx-30, cx+30
    cylinder(37,d=10,at=(x0,50,8),name=tag+'a')
    s=sketch('XY',offset=45); s.circle((x0,50),d=10)
    cmd('solid.revolve',sketch=s,axis={'origin':[x0+15,50,45],'dir':[0,1,0]},angle='90 deg',operation='new',body_name=tag+'b')
    cylinder(30,d=10,at=(x0+15,50,60),axis='X',name=tag+'c')
    s=sketch('XY',offset=45); s.circle((x1,50),d=10)
    cmd('solid.revolve',sketch=s,axis={'origin':[x1-15,50,45],'dir':[0,1,0]},angle='90 deg',operation='new',body_name=tag+'d')
    cylinder(37,d=10,at=(x1,50,8),name=tag+'e')
    combine(tag+'a',[tag+c for c in 'bcde'],op='join')
    return tag+'a'
if S >= 1:
    h=handle('H'); rename(h,'Handle')
if S >= 3:
    h2=handle('K'); move(h2,by=(-cx,-50,0)); move(h2,rotate=('Z',90)); move(h2,by=(cx,50,0))
    combine('Handle',[h2],op='join')
measure()
