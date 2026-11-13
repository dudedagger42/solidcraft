with part('Inlet'):
    box(80,80,6,center=(0,0,3),name='Inlet'); box(54,34,6,center=(0,0,3),op='cut',into='Inlet')
    for x in (-32,32):
        for y in (-32,32): hole(at=(x,y,6),d=6)
with part('Duct'):
    a=sketch('XY',offset=6); a.rect_center((0,0),54,34); a._finish()
    b=sketch('XY',offset=66); b.circle((0,0),d=44); b._finish()
    cmd('solid.loft',sections=[{'sketch':a.id},{'sketch':b.id}],operation='new',body_name='Duct')
    c=sketch('XY',offset=6); c.rect_center((0,0),50,30); c._finish()
    e=sketch('XY',offset=66); e.circle((0,0),d=40); e._finish()
    cmd('solid.loft',sections=[{'sketch':c.id},{'sketch':e.id}],operation='cut',targets=['Duct'])
with part('Outlet'):
    cylinder(15,d=44,at=(0,0,66),name='Outlet'); cylinder(15,d=40,at=(0,0,66),op='cut',into='Outlet')
