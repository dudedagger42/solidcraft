with part('Housing'):
    box(120,80,60,center=(0,0,30),name='Housing'); hollow(4,open='top')
    for x in (-30,30): hole(at=(x,-40,30),d=12.5); hole(at=(x,40,30),d=12.5)
with part('Lid'):
    box(120,80,5,center=(0,0,62.5),name='Lid')
    for x in (-52,52):
        for y in (-32,32): hole(at=(x,y,65),d=6)
with part('ShaftA'): cylinder(100,d=12,at=(-30,-50,30),axis='Y',name='ShaftA')
with part('ShaftB'): cylinder(100,d=12,at=(30,-50,30),axis='Y',name='ShaftB')
with part('Gear'):
    cylinder(10,d=50,at=(-30,-5,30),axis='Y',name='Gear'); cylinder(10,d=12,at=(-30,-5,30),axis='Y',op='cut',into='Gear')
