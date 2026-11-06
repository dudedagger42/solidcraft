box(100,60,10,name='Plate')
for x,y in [(10,10),(90,10),(10,50),(90,50)]: hole(at=(x,y,10),d=6)
box(40,20,4,at=(30,20,6),op='cut',into='Plate')
