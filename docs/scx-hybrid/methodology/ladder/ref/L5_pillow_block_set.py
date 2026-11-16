with part('Base'):
    box(120,60,10,at=(-60,-30,0),name='Base')
    for x,y in [(-50,-20),(50,-20),(-50,20),(50,20)]: hole(at=(x,y,10),d=10)
with part('Block'):
    box(60,30,40,at=(-30,-15,10),name='Block'); hole(at=(0,-15,30),d=25)
    for x in (-20,20): hole(at=(x,0,50),d=8)
for x in (-20,20): hole(at=(x,0,10),d=8,into='Base') if False else None
with part('Shaft'): cylinder(80,d=24,at=(0,-40,30),axis='Y',name='Shaft')
