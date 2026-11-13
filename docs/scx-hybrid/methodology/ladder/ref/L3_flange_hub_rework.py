cylinder(10,d=80,name='Hub'); cylinder(25,d=40,at=(0,0,10),op='join',into='Hub')
hole(at=(0,0,35),d=24)
for a in (30,90,150,210,270,330): hole(at=(30*cos(radians(a)),30*sin(radians(a)),10),d=8)
box(5,6,35,at=(10,-3,0),op='cut',into='Hub')
