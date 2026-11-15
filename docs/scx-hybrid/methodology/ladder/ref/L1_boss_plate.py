box(140,90,8,name='BossPlate')
for x in (25,55,85,115):
    for y in (20,45,70): cylinder(12,d=14,at=(x,y,8),op='join',into='BossPlate')
for y0 in (30.5,55.5): box(120,4,6,at=(10,y0,8),op='join',into='BossPlate')
for x in (25,55,85,115):
    for y in (20,45,70): hole(at=(x,y,20),d=6)
for x,y in [(6,6),(134,6),(6,84),(134,84)]: hole(at=(x,y,8),d=5)
