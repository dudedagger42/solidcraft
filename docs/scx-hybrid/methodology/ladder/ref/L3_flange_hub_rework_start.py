cylinder(10,d=80,name='Hub'); cylinder(15,d=40,at=(0,0,10),op='join',into='Hub')
hole(at=(0,0,25),d=20)
for a in (0,90,180,270): hole(at=(30*cos(radians(a)),30*sin(radians(a)),10),d=8)
