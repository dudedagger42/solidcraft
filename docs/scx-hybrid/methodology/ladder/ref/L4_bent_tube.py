def path(d, tag):
    def disc(plane, off, c):
        s=sketch(plane,offset=off); s.circle(c,d=d); return s
    disc('YZ',0,(0,0)).extrude(60,name=tag+'0')
    s=disc('YZ',60,(0,0)); s._finish()
    cmd('solid.revolve',sketch=s.id,axis={'origin':[60,30,0],'dir':[0,0,1]},angle='90 deg',operation='new',body_name=tag+'1')
    disc('XZ',30,(90,0)).extrude(60,name=tag+'2')
    s=disc('XZ',90,(90,0)); s._finish()
    cmd('solid.revolve',sketch=s.id,axis={'origin':[90,90,30],'dir':[1,0,0]},angle='-90 deg',operation='new',body_name=tag+'3')
    disc('XY',30,(90,120)).extrude(70,name=tag+'4')
    combine(tag+'0',[tag+str(i) for i in range(1,5)],op='join')
path(16,'Tube'); path(10,'Core')
combine('Tube0',['Core0'],op='cut'); rename('Tube0','Tube')
