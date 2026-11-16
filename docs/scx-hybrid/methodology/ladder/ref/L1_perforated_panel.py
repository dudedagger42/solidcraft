box(160,100,6,name='Panel')
for x in (25,45,65,85,105,125,145):
    for y in (30,45,60,75): hole(at=(x,y,6),d=8)
for x,y in [(8,8),(152,8),(8,92),(152,92)]:
    hole(at=(x,y,6),d=11,depth=4); hole(at=(x,y,2),d=6.6)
box(30,12,3,at=(65,8,3),op='cut'); box(30,12,3,at=(65,80,3),op='cut')
