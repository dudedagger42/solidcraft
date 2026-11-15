remove('Lid')
with part('Lid'):
    box(120,80,8,center=(0,0,64),name='Lid')
    for x in (-52,52):
        for y in (-32,32): hole(at=(x,y,68),d=6)
    hole(at=(0,0,68),d=20)
