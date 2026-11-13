S=1
# full state for step S (set by the line prepended)
L = 160 if S >= 1 else 100
box(L,80,12,name='MotorPlate'); chamfer([(L/2,0,12)],8)
bosses = [50,120] if S >= 2 else [50]
for bx in bosses:
    cylinder(6,d=40,at=(bx,40,12),op='join',into='MotorPlate')
for bx in bosses:
    hole(at=(bx,40,18),d=22)
    for a in (45,135,225,315): hole(at=(bx+15.5*cos(radians(a)),40+15.5*sin(radians(a)),18),d=5)
for x in (12, 148 if S >= 3 else 88): hole(at=(x,70,12),d=8)
for x in (25, 135 if S >= 3 else 75): hole(at=(x,4,8),d=4,depth=6)
if S >= 4: fillet([(0,80,6),(L,80,6)],10)
