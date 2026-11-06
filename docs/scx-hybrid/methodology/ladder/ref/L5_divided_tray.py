box(150,100,25,center=(0,0,12.5),name='Tray'); hollow(3,open='top')
w=(144-4)/3
for i in (1,2): box(2,94,22,at=(-72+i*w+(i-1)*2,-47,3),op='join',into='Tray')
for i in range(3): hole(at=(-72+i*(w+2)+w/2,0,3),d=10)
