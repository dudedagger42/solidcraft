S=3
cylinder(4,d=120,name='Impeller')
HT = 30 if S >= 3 else 20
cylinder(HT,d=30,op='join',into='Impeller')
N = 6 if S >= 5 else 8
if S >= 1:
    a=sketch('YZ',offset=14); a.rect_center((0,12),3,16); b=sketch('YZ',offset=58); b.rect_center((0,8),3,8)
    loft([a,b],name='Bl0')
    names=['Bl0']
    for i in range(1,N):
        move('Bl0',rotate=('Z',360/N*i),copy=True)
    combine('Impeller',[n for n in measure() if n.startswith('Bl0')],op='join')
hole(at=(0,0,HT),d=8)
if S >= 2:
    for i in range(N):
        a=radians(180/N+360/N*i); hole(at=(45*cos(a),45*sin(a),4),d=5)
if S >= 3:
    hole(at=(0,15,25),d=3,depth=11)
if S >= 4:
    with part('Shaft'): cylinder(60,d=8,at=(0,0,-20),name='Shaft')
