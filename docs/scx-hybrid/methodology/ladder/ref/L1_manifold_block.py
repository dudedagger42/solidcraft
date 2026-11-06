box(120,80,50,name='Manifold')
for x in (20,40,60,80,100):
    for y in (20,60): hole(at=(x,y,50),d=10,depth=20)
    hole(at=(x,0,15),d=8,depth=15)
for x in (30,50,70,90): hole(at=(x,80,15),d=8,depth=15)
for z in (15,35): hole(at=(0,40,z),d=12,depth=25)
for y in (20,40,60): cylinder(6,d=16,at=(120,y,25),axis='X',op='join',into='Manifold')
for y in (20,40,60): hole(at=(126,y,25),d=6,depth=26)
