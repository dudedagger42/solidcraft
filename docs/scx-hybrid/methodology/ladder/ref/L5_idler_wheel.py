cylinder(20,d=60,at=(0,0,5),name='Wheel'); cylinder(30,d=30,op='join',into='Wheel')
s=sketch('XZ'); s.rect((27,10),(30,20)); s.revolve('Z',op='cut',into='Wheel')
hole(at=(0,0,30),d=12)
for i in range(6): hole(at=(20*cos(radians(60*i)),20*sin(radians(60*i)),25),d=8)
