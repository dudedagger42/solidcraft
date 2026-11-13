box(20,80,4,at=(60,-40,0),op='join',into='Housing'); box(20,80,4,at=(-80,-40,0),op='join',into='Housing')
for x in (-70,70):
    for y in (-25,25): hole(at=(x,y,4),d=8)
