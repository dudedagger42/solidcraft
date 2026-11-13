with part('Base'):
    box(120,60,10,at=(-60,-30,0),name='Base'); hole(at=(-20,0,10),d=10)
with part('Post'): cylinder(70,d=16,at=(20,0,10),name='Post')
with part('Collar'):
    cylinder(12,d=28,at=(20,0,10),name='Collar'); cylinder(12,d=16,at=(20,0,10),op='cut',into='Collar')
