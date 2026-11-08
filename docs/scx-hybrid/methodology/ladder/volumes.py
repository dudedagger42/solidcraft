"""Exact volumes and bounding boxes for the ladder benchmark tasks.

Run:  python volumes.py          -> writes tasks.json next to this file
      python volumes.py --check  -> also cross-checks every body by grid sampling

All units mm. Every volume is a closed form (prisms, cylinders, Pappus for
revolves and swept tubes, prismatoid formula for the ruled loft, exact
circular-segment area for the keyway). The --check pass integrates a
point-membership function of each body on a fine grid; it is only a sanity
check of the formulas (agreement to about 0.1-0.3 %), not the reference.
"""
import json, math, os, sys

pi = math.pi
S2 = math.sqrt(2.0)
S3 = math.sqrt(3.0)
HERE = os.path.dirname(os.path.abspath(__file__))


def cyl(d, h):
    return pi * (d / 2.0) ** 2 * h


def r4(x):
    return round(x + 0.0, 4)


def bb(lo, hi):
    return [[r4(v) for v in lo], [r4(v) for v in hi]]


def revolve_polygon(pts):
    """Volume of a closed polygon in the (r, z) half-plane revolved 360 deg about z.
    Pappus: V = 2*pi * integral(r dA); for a polygon
    integral(r dA) = 1/6 * sum (r_i + r_j) * (r_i z_j - r_j z_i)."""
    s = 0.0
    n = len(pts)
    for i in range(n):
        r1, z1 = pts[i]
        r2, z2 = pts[(i + 1) % n]
        s += (r1 + r2) * (r1 * z2 - r2 * z1)
    return abs(2 * pi * s / 6.0)


def tilted_cyl_bbox(c0, a, h, r):
    """AABB of a cylinder: base centre c0, unit axis a, height h, radius r."""
    c1 = [c0[i] + h * a[i] for i in range(3)]
    lo, hi = [], []
    for i in range(3):
        e = r * math.sqrt(max(0.0, 1 - a[i] ** 2))
        lo.append(min(c0[i], c1[i]) - e)
        hi.append(max(c0[i], c1[i]) + e)
    return lo, hi


tasks = {}

# ---------------------------------------------------------------- L1
# L1_perforated_panel
xs = [25, 45, 65, 85, 105, 125, 145]
ys = [30, 45, 60, 75]
v = 160 * 100 * 6
v -= len(xs) * len(ys) * cyl(8, 6)
v -= 4 * (cyl(6.6, 6) + (cyl(11, 4) - cyl(6.6, 4)))
v -= 2 * 30 * 12 * 3
tasks["L1_perforated_panel"] = dict(
    level=1,
    spec=("A flat panel 160 mm (X) by 100 mm (Y) by 6 mm thick (Z), spanning x 0..160, y 0..100, z 0..6. "
          "(a) 28 plain through holes of diameter 8 mm along Z, on a grid: every combination of x = 25, 45, 65, 85, 105, 125, 145 "
          "and y = 30, 45, 60, 75. "
          "(b) Four counterbored through holes, one near each corner, centred at (8,8), (152,8), (8,92) and (152,92): "
          "through hole diameter 6.6 mm, counterbore diameter 11 mm and 4 mm deep from the top face (the counterbore floor is flat at z = 2). "
          "(c) Two rectangular blind pockets cut down from the top face, each 3 mm deep with a flat floor at z = 3 and square (unrounded) corners: "
          "one spanning x 65..95, y 8..20, the other x 65..95, y 80..92. "
          "One solid body named Panel."),
    bodies=[dict(name="Panel", volume=r4(v), bbox=bb([0, 0, 0], [160, 100, 6]))],
    math="160*100*6 - 28*pi*4^2*6 - 4*(pi*3.3^2*6 + pi*(5.5^2-3.3^2)*4) - 2*30*12*3",
)

# L1_manifold_block
v = 120 * 80 * 50
v -= 10 * cyl(10, 20)      # top blind holes
v -= 5 * cyl(8, 15)        # front
v -= 4 * cyl(8, 15)        # back
v -= 2 * cyl(12, 25)       # left
v += 3 * cyl(16, 6)        # right bosses
v -= 3 * cyl(6, 26)        # holes through bosses, 6 in boss + 20 in block
tasks["L1_manifold_block"] = dict(
    level=1,
    spec=("A rectangular block spanning x 0..120, y 0..80, z 0..50, with these features (all blind holes have flat bottoms, no drill-point cone): "
          "(a) Top face (z=50): ten blind holes of diameter 10 mm, 20 mm deep (down to z=30), centred at x = 20, 40, 60, 80, 100 for each of y = 20 and y = 60. "
          "(b) Front face (y=0): five blind holes of diameter 8 mm, 15 mm deep along +Y (to y=15), axes at z = 15 and x = 20, 40, 60, 80, 100. "
          "(c) Back face (y=80): four blind holes of diameter 8 mm, 15 mm deep along -Y (to y=65), axes at z = 15 and x = 30, 50, 70, 90. "
          "(d) Left face (x=0): two blind holes of diameter 12 mm, 25 mm deep along +X (to x=25), axes at y = 40 with z = 15 and z = 35. "
          "(e) Right face (x=120): three cylindrical bosses of diameter 16 mm protruding 6 mm in +X (to x=126), axes along X at z = 25 and y = 20, 40, 60. "
          "(f) Each boss has a coaxial blind hole of diameter 6 mm drilled from the boss end face, 26 mm deep along -X (from x=126 to x=100). "
          "One solid body named Manifold."),
    bodies=[dict(name="Manifold", volume=r4(v), bbox=bb([0, 0, 0], [126, 80, 50]))],
    math="120*80*50 - 10*pi*5^2*20 - 5*pi*4^2*15 - 4*pi*4^2*15 - 2*pi*6^2*25 + 3*pi*8^2*6 - 3*pi*3^2*26 (no features intersect each other)",
)

# L1_boss_plate
v = 140 * 90 * 8
v -= 4 * cyl(5, 8)
v += 12 * cyl(14, 12)
v -= 12 * cyl(6, 20)
v += 2 * 120 * 4 * 6
tasks["L1_boss_plate"] = dict(
    level=1,
    spec=("A base plate spanning x 0..140, y 0..90, z 0..8. "
          "(a) Twelve cylindrical bosses of diameter 14 mm standing on the top face, each 12 mm tall (z 8..20), centred at every combination of "
          "x = 25, 55, 85, 115 and y = 20, 45, 70. "
          "(b) Each boss has a coaxial through hole of diameter 6 mm running along Z through the boss and the plate (z 0..20). "
          "(c) Four plain through holes of diameter 5 mm in the plate at (6,6), (134,6), (6,84) and (134,84). "
          "(d) Two straight rectangular ribs on the top face, each 6 mm tall (z 8..14) and spanning x 10..130: "
          "one spanning y 30.5..34.5, the other y 55.5..59.5. "
          "Everything is fused into one solid body named BossPlate."),
    bodies=[dict(name="BossPlate", volume=r4(v), bbox=bb([0, 0, 0], [140, 90, 20]))],
    math="140*90*8 - 4*pi*2.5^2*8 + 12*pi*7^2*12 - 12*pi*3^2*20 + 2*120*4*6 (ribs do not touch bosses)",
)

# ---------------------------------------------------------------- L2
# L2_ramp_block
area = 80 * 40 - 0.5 * 30 * 30          # pentagon (0,0),(80,0),(80,10),(50,40),(0,40)
v = area * 50
v -= 2 * cyl(10, 40)
v -= 14 * 10 * 15
v += cyl(20, 15)
v -= cyl(8, 25)
a = [1 / S2, 0, 1 / S2]
c0 = [65, 25, 25]
blo, bhi = tilted_cyl_bbox(c0, a, 15, 10)
lo = [min(0, blo[0]), min(0, blo[1]), min(0, blo[2])]
hi = [max(80, bhi[0]), max(50, bhi[1]), max(40, bhi[2])]
tasks["L2_ramp_block"] = dict(
    level=2,
    spec=("A block whose side profile in the XZ plane is the polygon (0,0), (80,0), (80,10), (50,40), (0,40) (x,z), extruded 50 mm along Y from y=0 to y=50. "
          "It therefore has a flat top (z=40, x 0..50), a vertical back face at x=0, a short vertical front face at x=80 (z 0..10), "
          "and a 45-degree sloped face running from the edge at x=80, z=10 up to the edge at x=50, z=40, facing up and towards +X. "
          "(a) A cylindrical boss of diameter 20 mm stands on the sloped face: its axis is perpendicular to the sloped face and passes through the centre of that face "
          "(midway along the slope and midway across its 50 mm width), and it sticks out 15 mm from the face, measured along its axis. "
          "(b) A blind hole of diameter 8 mm, coaxial with the boss, is drilled from the boss's end face 25 mm deep along the axis (flat bottom), "
          "so it goes through the boss and 10 mm into the block. "
          "(c) Two through holes of diameter 10 mm go straight down (along Z) through the flat top, each 12 mm in from the back face and 12 mm in from its nearer side face (y=0 or y=50). "
          "(d) A rectangular pocket is cut into the back face, centred on that face: 14 mm wide (Y) by 10 mm tall (Z), 15 mm deep (along +X), flat floor, square corners. "
          "One solid body named RampBlock."),
    bodies=[dict(name="RampBlock", volume=r4(v), bbox=bb(lo, hi))],
    math=("profile area 80*40 - 30*30/2 = 2750, *50; - 2*pi*5^2*40 (vertical holes); - 14*10*15 (pocket); "
          "+ pi*10^2*15 (boss, entirely outside the half-space x+z<=90 so no overlap); - pi*4^2*25 (hole all in material). "
          "Face centre (65,25,25), axis (1,0,1)/sqrt2; boss top centre = (65+15/sqrt2, 25, 25+15/sqrt2); bbox max x = z+40 = 65 + 15/sqrt2 + 10/sqrt2 = 65 + 25/sqrt2."),
)

# L2_hex_hub
hexA = 800 * S3                      # across flats 40: area = 2*sqrt3*(40/2)^2
corner = 40 / S3
v = hexA * 30
v -= cyl(16, 30)
v -= (cyl(24, 6) - cyl(16, 6))
v -= 6 * cyl(6, 8)
v -= 2 * cyl(5, 6)
tasks["L2_hex_hub"] = dict(
    level=2,
    spec=("A regular hexagonal prism, 40 mm across the flats and 30 mm tall, standing on the XY plane (z 0..30) with its axis on the Z axis. "
          "Two of its side faces are parallel to the XZ plane (so the flats are at y = +20 and y = -20 and two corners lie on the X axis). "
          "(a) A through bore of diameter 16 mm on the axis. "
          "(b) A counterbore of diameter 24 mm, coaxial with the bore, 6 mm deep from the top face (flat floor). "
          "(c) On each of the six side faces, a blind hole of diameter 6 mm, 8 mm deep, drilled perpendicular to that face, centred on the face "
          "(midway along its width and at mid-height of the prism), flat bottom. "
          "(d) In the bottom face, two blind holes of diameter 5 mm, 6 mm deep (flat bottom), parallel to Z, both on the X axis, one on each side of the bore, "
          "each centred midway between the edge of the bore and the hexagon corner on that side. "
          "One solid body named HexHub."),
    bodies=[dict(name="HexHub", volume=r4(v), bbox=bb([-corner, -20, 0], [corner, 20, 30]))],
    math=("hexagon area 800*sqrt3 (across flats 40), *30; - pi*8^2*30; - pi*(12^2-8^2)*6; - 6*pi*3^2*8 (start on the flat face, end at radius 12, clear of the bore r8 and counterbore z>=24); "
          "- 2*pi*2.5^2*6 (centres at x = +-(8 + 40/sqrt3)/2 = +-15.547). Corners at x = +-40/sqrt3."),
)

# L2_chamfered_motor_plate
v = 100 * 80 * 12
v -= 0.5 * 8 * 8 * 100
v += cyl(40, 6)
v -= cyl(22, 18)
v -= 4 * cyl(5, 18)
v -= 2 * cyl(8, 12)
v -= 2 * cyl(4, 6)
tasks["L2_chamfered_motor_plate"] = dict(
    level=2,
    spec=("A plate spanning x 0..100, y 0..80, z 0..12. Its top front edge (the edge along X at y=0, z=12) has an 8 mm x 45-degree chamfer, "
          "so the chamfer face runs from y=0, z=4 up to y=8, z=12 over the full length in X. "
          "(a) A cylindrical boss of diameter 40 mm, 6 mm tall, stands on the top face, centred in X and centred between the front (y=0) and back (y=80) faces of the plate. "
          "(b) A through bore of diameter 22 mm, coaxial with the boss, through the boss and the plate. "
          "(c) Four through holes of diameter 5 mm through the boss and the plate, on a 31 mm diameter circle around the boss axis, at 45, 135, 225 and 315 degrees measured from +X. "
          "(d) Two through holes of diameter 8 mm along Z, each 10 mm in from the back face and 12 mm in from its nearer end face (x=0 or x=100). "
          "(e) On the chamfer face, two blind holes of diameter 4 mm drilled perpendicular to the chamfer face, 6 mm deep (flat bottom), each centred midway across the chamfer face's width, "
          "one 25 mm in from the x=0 end and one 25 mm in from the x=100 end. "
          "One solid body named MotorPlate."),
    bodies=[dict(name="MotorPlate", volume=r4(v), bbox=bb([0, 0, 0], [100, 80, 18]))],
    math=("100*80*12 - 8*8/2*100 (chamfer) + pi*20^2*6 - pi*11^2*18 - 4*pi*2.5^2*18 - 2*pi*4^2*12 - 2*pi*2^2*6. "
          "Chamfer holes start on the chamfer plane, axis normal to it, so each removes exactly pi*r^2*depth (bottom centre at y 8.24, z 3.76, inside)."),
)

# ---------------------------------------------------------------- L3
# L3_plate_rework
v0 = 100 * 60 * 10 - 4 * cyl(6, 10) - 40 * 20 * 4
v = 130 * 60 * 10 - 6 * cyl(8, 10) - cyl(20, 10)
start = ("One solid body named Plate: a box spanning x 0..100, y 0..60, z 0..10, with four through holes of diameter 6 mm along Z at (10,10), (90,10), (10,50), (90,50), "
         "and a rectangular pocket 4 mm deep from the top face spanning x 30..70, y 20..40, z 6..10 (flat floor, square corners). "
         f"Volume {r4(v0)} mm3, bbox [[0,0,0],[100,60,10]].")
tasks["L3_plate_rework"] = dict(
    level=3,
    start=start,
    spec=("The current design is: " + start + " Make these changes to Plate: "
          "1) Lengthen the plate to 130 mm in X by moving its +X end face out; the x=0 end stays put (new extent x 0..130). "
          "2) Move the two right-hand holes so they stay 10 mm from the right end: their centres become (120,10) and (120,50). "
          "3) Enlarge all four corner holes from 6 mm to 8 mm diameter. "
          "4) Remove the pocket completely (the top face becomes flat again). "
          "5) Add a through hole of diameter 20 mm at the centre of the lengthened plate, (65,30). "
          "6) Add two more through holes of diameter 8 mm at (65,10) and (65,50). "
          "The result is still one body named Plate."),
    bodies=[dict(name="Plate", volume=r4(v), bbox=bb([0, 0, 0], [130, 60, 10]))],
    math="start: 100*60*10 - 4*pi*3^2*10 - 40*20*4. result: 130*60*10 - 6*pi*4^2*10 - pi*10^2*10",
)

# L3_assembly_rework
vb0, vp0, vc0 = 100 * 60 * 10, cyl(16, 50), cyl(30, 5)
vb = 120 * 60 * 10 - cyl(10, 10)
vp = cyl(16, 70)
vcol = cyl(28, 12) - cyl(16, 12)
start = ("Three separate solid bodies (separate parts, not fused): "
         f"Base, a box spanning x -50..50, y -30..30, z 0..10 (volume {r4(vb0)}, bbox [[-50,-30,0],[50,30,10]]); "
         f"Post, a solid cylinder of diameter 16 mm with its axis vertical through (x=-20, y=0), spanning z 10..60 (volume {r4(vp0)}, bbox [[-28,-8,10],[-12,8,60]]); "
         f"Cap, a solid disc of diameter 30 mm, coaxial with Post, spanning z 60..65 (volume {r4(vc0)}, bbox [[-35,-15,60],[-5,15,65]]).")
tasks["L3_assembly_rework"] = dict(
    level=3,
    start=start,
    spec=("The current design is: " + start + " Make these changes: "
          "1) Delete the Cap body. "
          "2) Move Post so its axis passes through (x=20, y=0) instead of (x=-20, y=0); its bottom stays at z=10. "
          "3) Make Post 70 mm tall (z 10..80); its diameter stays 16 mm. "
          "4) Lengthen Base to 120 mm in X, symmetrically (x -60..60); its Y and Z extents are unchanged. "
          "5) Cut a through hole of diameter 10 mm along Z in Base at (x=-20, y=0), where the post used to stand. "
          "6) Add a new separate body named Collar: a tube of outside diameter 28 mm and inside diameter 16 mm, 12 mm tall, coaxial with the moved Post and resting on the top of Base (z 10..22). "
          "Keep Base, Post and Collar as three separate bodies; do not fuse them. Nothing else may remain."),
    bodies=[
        dict(name="Base", volume=r4(vb), bbox=bb([-60, -30, 0], [60, 30, 10])),
        dict(name="Post", volume=r4(vp), bbox=bb([12, -8, 10], [28, 8, 80])),
        dict(name="Collar", volume=r4(vcol), bbox=bb([6, -14, 10], [34, 14, 22])),
    ],
    math="Base 120*60*10 - pi*5^2*10; Post pi*8^2*70; Collar pi*(14^2-8^2)*12",
)

# L3_flange_hub_rework
v0 = cyl(80, 10) + cyl(40, 15) - cyl(20, 25) - 4 * cyl(8, 10)
key_area = 15 * 6 - (3 * math.sqrt(135) + 144 * math.asin(0.25))
v = cyl(80, 10) + cyl(40, 25) - cyl(24, 35) - key_area * 35 - 6 * cyl(8, 10)
start = ("One solid body named Hub, axis on the Z axis: a disc of diameter 80 mm spanning z 0..10, with a coaxial cylindrical boss of diameter 40 mm on top spanning z 10..25, "
         "a through bore of diameter 20 mm on the axis (z 0..25), and four through holes of diameter 8 mm in the disc on a 60 mm diameter circle at 0, 90, 180 and 270 degrees from +X. "
         f"Volume {r4(v0)} mm3, bbox [[-40,-40,0],[40,40,25]].")
tasks["L3_flange_hub_rework"] = dict(
    level=3,
    start=start,
    spec=("The current design is: " + start + " Make these changes to Hub: "
          "1) Replace the four bolt holes with six through holes of diameter 8 mm on the same 60 mm circle, at 30, 90, 150, 210, 270 and 330 degrees from +X (the old holes at 0 and 180 degrees are filled; 90 and 270 stay). "
          "2) Make the boss taller: its top moves from z=25 to z=35 (boss spans z 10..35). "
          "3) Enlarge the bore to 24 mm diameter, still running through the full height (z 0..35). "
          "4) Cut a keyway along the full length of the bore (z 0..35): the region with -3 <= y <= 3 and x from the bore wall out to x = 15 "
          "(a straight-walled slot 6 mm wide on the +X side of the bore whose flat end is the plane x = 15). "
          "The result is still one body named Hub."),
    bodies=[dict(name="Hub", volume=r4(v), bbox=bb([-40, -40, 0], [40, 40, 35]))],
    math=("pi*40^2*10 + pi*20^2*25 - pi*12^2*35 - A_key*35 - 6*pi*4^2*10, where A_key = area of {0<=x<=15, |y|<=3} outside the circle r=12 "
          "= 90 - (3*sqrt(135) + 144*asin(1/4)) (the strip |y|<=3 inside the circle on x>=0 is integral sqrt(144-y^2) dy from -3 to 3)."),
)

# ---------------------------------------------------------------- L4
# L4_v_pulley
prof = [(10, 0), (18, 0), (18, 14), (30, 14), (30, 6), (50, 6), (50, 10), (38, 16), (38, 20), (50, 26),
        (50, 30), (30, 30), (30, 22), (18, 22), (18, 36), (10, 36)]
v = revolve_polygon(prof) - 6 * cyl(8, 8)
tasks["L4_v_pulley"] = dict(
    level=4,
    spec=("A V-groove pulley made by revolving a profile 360 degrees about the Z axis, then adding holes. The half cross-section in the (r, z) plane, "
          "r = distance from the Z axis, is the closed polygon (r,z): (10,0), (18,0), (18,14), (30,14), (30,6), (50,6), (50,10), (38,16), (38,20), (50,26), "
          "(50,30), (30,30), (30,22), (18,22), (18,36), (10,36). "
          "In words: a hub from r 10 to 18 (so a 20 mm bore) spanning z 0..36; a web from r 18 to 30 spanning z 14..22; a rim from r 30 to 50 spanning z 6..30; "
          "and a V-groove in the rim whose sides run from r=50 at z=10 and z=26 down to a flat bottom at r=38 spanning z 16..20. "
          "Then cut six through holes of diameter 8 mm parallel to Z through the web, on a 48 mm diameter circle, evenly spaced, the first on the +X axis. "
          "One solid body named Pulley."),
    bodies=[dict(name="Pulley", volume=r4(v), bbox=bb([-50, -50, 0], [50, 50, 36]))],
    math="Pappus: V = 2*pi*integral(r dA) with integral(r dA) = 1/6*sum (r_i+r_j)(r_i z_j - r_j z_i) over the polygon; minus 6*pi*4^2*8 (holes r 20..28 lie inside the web r 18..30).",
)

# L4_bent_tube
L = 60 + pi * 30 / 2 + 60 + pi * 30 / 2 + 70
v = (cyl(16, 1) - cyl(10, 1)) * L
tasks["L4_bent_tube"] = dict(
    level=4,
    spec=("A bent hollow tube of outside diameter 16 mm and inside diameter 10 mm (open at both ends), swept along this centreline path: "
          "start at (0,0,0) and run straight along +X to (60,0,0); then a 90-degree arc of centreline radius 30 mm in the XY plane, centre (60,30,0), turning towards +Y and ending at (90,30,0) heading +Y; "
          "then straight along +Y to (90,90,0); then a 90-degree arc of centreline radius 30 mm in the plane x=90, centre (90,90,30), turning upwards and ending at (90,120,30) heading +Z; "
          "then straight along +Z to (90,120,100). The tube cross-section stays perpendicular to the path throughout, and the end faces are flat and perpendicular to the path "
          "(the start face lies in the plane x=0, the end face in the plane z=100). One solid body named Tube."),
    bodies=[dict(name="Tube", volume=r4(v), bbox=bb([0, -8, -8], [98, 128, 100]))],
    math=("annulus area pi*(8^2-5^2) times centreline length 60 + 15*pi + 60 + 15*pi + 70 = 190 + 30*pi (exact by Pappus for the torus segments since bend radius 30 > tube radius 8). "
          "bbox: arcs are monotone 90-degree turns between axis directions, so the extremes are on the straight runs and arc ends: x max 60+30+8, y max 90+30+8, z max 100, mins -8 / x 0."),
)

# L4_loft_on_rounded_base
A1 = 60 * 20
A2 = 20 * 60
Am = 40 * 40
vloft = 40 / 6 * (A1 + A2 + 4 * Am)
vbase = (80 * 80 - (4 - pi) * 12 ** 2) * 10
v = vbase + vloft - cyl(12, 50)
tasks["L4_loft_on_rounded_base"] = dict(
    level=4,
    spec=("A square base plate 80 x 80 mm, 10 mm thick, centred on the Z axis (x -40..40, y -40..40, z 0..10); its four vertical edges are filleted with radius 12 mm "
          "(only those four edges; all other edges of the plate stay sharp). On top of the plate, a straight-sided (ruled) loft rises from a rectangle 60 mm in X by 20 mm in Y "
          "lying on the plate's top face (z=10) to a rectangle 20 mm in X by 60 mm in Y at z=50; both rectangles are centred on the Z axis, and each corner connects straight to the "
          "corresponding corner of the other rectangle (the +X+Y corner to the +X+Y corner, and so on), so all four side faces are flat. "
          "Finally a through hole of diameter 12 mm runs along the Z axis through the loft and the plate (z 0..50). Everything is one fused solid body named Transition."),
    bodies=[dict(name="Transition", volume=r4(v), bbox=bb([-40, -40, 0], [40, 40, 50]))],
    math=("base: (80^2 - (4-pi)*12^2)*10 (four parallel vertical edge fillets, no corner blends); loft: prismatoid h/6*(A1 + A2 + 4*Am) = 40/6*(1200 + 1200 + 4*40*40) "
          "(section is an a(z) x b(z) rectangle with a, b linear in z, so area is quadratic and Simpson is exact); minus pi*6^2*50."),
)

# ---------------------------------------------------------------- L5
# L5_divided_tray
comp = (144 - 4) / 3.0
v = 150 * 100 * 25 - 144 * 94 * 22 + 2 * (2 * 94 * 22) - 3 * cyl(10, 3)
tasks["L5_divided_tray"] = dict(
    level=5,
    spec=("Make an open-top rectangular tray, 150 mm long (X) by 100 mm wide (Y) by 25 mm tall on the outside, with a 3 mm wall all round and a 3 mm floor. "
          "Two 2 mm thick dividers run across the width, wall to wall and as tall as the walls, splitting the inside into three equal compartments along the length. "
          "Each compartment has a single 10 mm drain hole through the floor, centred in that compartment. No fillets or drafts. "
          "The tray sits with its underside on the XY plane, centred on the origin. One body named Tray."),
    bodies=[dict(name="Tray", volume=r4(v), bbox=bb([-75, -50, 0], [75, 50, 25]))],
    math=f"150*100*25 - 144*94*22 (cavity) + 2*(2*94*22) (dividers) - 3*pi*5^2*3. Compartments are (144-4)/3 = {comp:.4f} long; dividers centred at x = +-(comp/2 + 1) = +-24.3333; hole centres at x = 0 and +-(comp + 2) = +-48.6667, y = 0.",
)

# L5_idler_wheel
v = cyl(60, 20) - (cyl(60, 10) - cyl(54, 10)) + cyl(30, 10) - cyl(12, 30) - 6 * cyl(8, 20)
tasks["L5_idler_wheel"] = dict(
    level=5,
    spec=("A flat-belt idler wheel: 60 mm across and 20 mm wide, with a square-sided groove 10 mm wide and 3 mm deep running all the way round the middle of the rim. "
          "A 30 mm diameter hub sticks out 5 mm from each side face of the wheel, and a 12 mm bore runs through the hub. "
          "Six 8 mm lightening holes go through the wheel parallel to the axis, evenly spaced on a 40 mm circle, one of them on the +X side. "
          "The axis is the Z axis and the lowest face sits on the XY plane. One body named Wheel."),
    bodies=[dict(name="Wheel", volume=r4(v), bbox=bb([-30, -30, 0], [30, 30, 30]))],
    math="pi*30^2*20 (wheel z 5..25) - pi*(30^2-27^2)*10 (groove z 10..20) + pi*15^2*10 (hub stubs z 0..5 and 25..30) - pi*6^2*30 (bore) - 6*pi*4^2*20 (holes r 16..24, clear of hub r15 and groove floor r27)",
)

# L5_pillow_block_set
vbase = 120 * 60 * 10 - 2 * cyl(8, 10) - 4 * cyl(10, 10)
vblock = 60 * 30 * 40 - cyl(25, 30) - 2 * cyl(8, 40)
vshaft = cyl(24, 80)
tasks["L5_pillow_block_set"] = dict(
    level=5,
    spec=("Three separate parts (keep them as separate bodies, not fused). "
          "Base: a plate 120 x 60 x 10 mm lying flat with its underside on the XY plane, centred on the origin, long side along X, with a 10 mm hole through each corner, 10 mm in from both edges. "
          "Block: 60 mm long (X), 30 mm deep (Y) and 40 mm tall, standing on top of Base and centred on it. A 25 mm bore runs right through the block front to back, "
          "centred along its length, with its axis 20 mm above the top of the plate. Two 8 mm bolt holes go straight down through both the block and the plate, "
          "on the block's centreline across its depth, 10 mm in from each end of the block. "
          "Shaft: a plain round bar 1 mm smaller in diameter than the bore, 80 mm long, lying in the bore, coaxial with it and centred on the block."),
    bodies=[
        dict(name="Base", volume=r4(vbase), bbox=bb([-60, -30, 0], [60, 30, 10])),
        dict(name="Block", volume=r4(vblock), bbox=bb([-30, -15, 10], [30, 15, 50])),
        dict(name="Shaft", volume=r4(vshaft), bbox=bb([-12, -40, 18], [12, 40, 42])),
    ],
    math=("Base 120*60*10 - 2*pi*4^2*10 (bolt holes at x=+-20) - 4*pi*5^2*10 (corner holes at (+-50,+-20)); "
          "Block 60*30*40 - pi*12.5^2*30 (bore along Y at z=30) - 2*pi*4^2*40 (bolt holes x=+-20, clear of bore |x|<=12.5); Shaft pi*12^2*80 along Y at x=0, z=30."),
)


# ---------------------------------------------------------------- grid cross-check
def check():
    import numpy as np

    def integrate(inside, lo, hi, h=0.2):
        xs = np.arange(lo[0] + h * 0.3719, hi[0], h)
        ys = np.arange(lo[1] + h * 0.4137, hi[1], h)
        zs = np.arange(lo[2] + h * 0.2913, hi[2], h)
        X, Y = np.meshgrid(xs, ys, indexing="ij")
        n = 0
        for z in zs:
            n += int(np.count_nonzero(inside(X, Y, np.full_like(X, z))))
        return n * h ** 3

    def disk(X, Y, cx, cy, d):
        return (X - cx) ** 2 + (Y - cy) ** 2 < (d / 2) ** 2

    def box(X, Y, Z, x0, x1, y0, y1, z0, z1):
        return (X > x0) & (X < x1) & (Y > y0) & (Y < y1) & (Z > z0) & (Z < z1)

    def tcyl(X, Y, Z, c0, a, r, t0, t1):
        dx, dy, dz = X - c0[0], Y - c0[1], Z - c0[2]
        t = dx * a[0] + dy * a[1] + dz * a[2]
        rr = dx * dx + dy * dy + dz * dz - t * t
        return (t > t0) & (t < t1) & (rr < r * r)

    f = {}

    def panel(X, Y, Z):
        m = box(X, Y, Z, 0, 160, 0, 100, 0, 6)
        for x in [25, 45, 65, 85, 105, 125, 145]:
            for y in [30, 45, 60, 75]:
                m &= ~disk(X, Y, x, y, 8)
        for x, y in [(8, 8), (152, 8), (8, 92), (152, 92)]:
            m &= ~disk(X, Y, x, y, 6.6)
            m &= ~(disk(X, Y, x, y, 11) & (Z > 2))
        m &= ~box(X, Y, Z, 65, 95, 8, 20, 3, 99) & ~box(X, Y, Z, 65, 95, 80, 92, 3, 99)
        return m
    f["L1_perforated_panel"] = [panel]

    def manifold(X, Y, Z):
        m = box(X, Y, Z, 0, 120, 0, 80, 0, 50)
        for y in (25, 60):
            pass
        for x in [20, 40, 60, 80, 100]:
            for y in [20, 60]:
                m &= ~(disk(X, Y, x, y, 10) & (Z > 30))
            m &= ~(((X - x) ** 2 + (Z - 15) ** 2 < 16) & (Y < 15))
        for x in [30, 50, 70, 90]:
            m &= ~(((X - x) ** 2 + (Z - 15) ** 2 < 16) & (Y > 65))
        for z in [15, 35]:
            m &= ~(((Y - 40) ** 2 + (Z - z) ** 2 < 36) & (X < 25))
        for y in [20, 40, 60]:
            m |= ((Y - y) ** 2 + (Z - 25) ** 2 < 64) & (X >= 120) & (X < 126)
            m &= ~(((Y - y) ** 2 + (Z - 25) ** 2 < 9) & (X > 100))
        return m
    f["L1_manifold_block"] = [manifold]

    def bossplate(X, Y, Z):
        m = box(X, Y, Z, 0, 140, 0, 90, 0, 8)
        m |= box(X, Y, Z, 10, 130, 30.5, 34.5, 0, 14) | box(X, Y, Z, 10, 130, 55.5, 59.5, 0, 14)
        for x in [25, 55, 85, 115]:
            for y in [20, 45, 70]:
                m |= disk(X, Y, x, y, 14) & (Z > 0) & (Z < 20)
                m &= ~disk(X, Y, x, y, 6)
        for x, y in [(6, 6), (134, 6), (6, 84), (134, 84)]:
            m &= ~disk(X, Y, x, y, 5)
        return m
    f["L1_boss_plate"] = [bossplate]

    def ramp(X, Y, Z):
        a = (1 / S2, 0, 1 / S2)
        m = box(X, Y, Z, 0, 80, 0, 50, 0, 40) & (X + Z < 90)
        m &= ~disk(X, Y, 12, 12, 10) & ~disk(X, Y, 12, 38, 10)
        m &= ~box(X, Y, Z, -1, 15, 18, 32, 15, 25)
        m |= tcyl(X, Y, Z, (65, 25, 25), a, 10, 0, 15)
        m &= ~tcyl(X, Y, Z, (65, 25, 25), a, 4, -10, 15)
        return m
    f["L2_ramp_block"] = [ramp]

    def hexhub(X, Y, Z):
        m = (Z > 0) & (Z < 30)
        normals = [(math.cos(math.radians(90 + 60 * k)), math.sin(math.radians(90 + 60 * k))) for k in range(6)]
        for nx, ny in normals:
            m &= X * nx + Y * ny < 20
        m &= ~disk(X, Y, 0, 0, 16) & ~(disk(X, Y, 0, 0, 24) & (Z > 24))
        for nx, ny in normals:
            m &= ~tcyl(X, Y, Z, (20 * nx, 20 * ny, 15), (-nx, -ny, 0), 3, 0, 8)
        xc = (8 + 40 / S3) / 2
        m &= ~(disk(X, Y, xc, 0, 5) & (Z < 6)) & ~(disk(X, Y, -xc, 0, 5) & (Z < 6))
        return m
    f["L2_hex_hub"] = [hexhub]

    def motor(X, Y, Z):
        m = box(X, Y, Z, 0, 100, 0, 80, 0, 12) & (Z - Y < 4)
        m |= disk(X, Y, 50, 40, 40) & (Z > 0) & (Z < 18)
        m &= ~disk(X, Y, 50, 40, 22)
        for ang in (45, 135, 225, 315):
            m &= ~disk(X, Y, 50 + 15.5 * math.cos(math.radians(ang)), 40 + 15.5 * math.sin(math.radians(ang)), 5)
        m &= ~disk(X, Y, 12, 70, 8) & ~disk(X, Y, 88, 70, 8)
        for x in (25, 75):
            m &= ~tcyl(X, Y, Z, (x, 4, 8), (0, 1 / S2, -1 / S2), 2, 0, 6)
        return m
    f["L2_chamfered_motor_plate"] = [motor]

    def plate3(X, Y, Z):
        m = box(X, Y, Z, 0, 130, 0, 60, 0, 10)
        for x, y in [(10, 10), (120, 10), (10, 50), (120, 50), (65, 10), (65, 50)]:
            m &= ~disk(X, Y, x, y, 8)
        return m & ~disk(X, Y, 65, 30, 20)
    f["L3_plate_rework"] = [plate3]

    f["L3_assembly_rework"] = [
        lambda X, Y, Z: box(X, Y, Z, -60, 60, -30, 30, 0, 10) & ~disk(X, Y, -20, 0, 10),
        lambda X, Y, Z: disk(X, Y, 20, 0, 16) & (Z > 10) & (Z < 80),
        lambda X, Y, Z: disk(X, Y, 20, 0, 28) & ~disk(X, Y, 20, 0, 16) & (Z > 10) & (Z < 22),
    ]

    def hub(X, Y, Z):
        m = (disk(X, Y, 0, 0, 80) & (Z > 0) & (Z < 10)) | (disk(X, Y, 0, 0, 40) & (Z > 0) & (Z < 35))
        m &= ~disk(X, Y, 0, 0, 24) & ~((X > 0) & (X < 15) & (np.abs(Y) < 3))
        for ang in range(30, 360, 60):
            m &= ~disk(X, Y, 30 * math.cos(math.radians(ang)), 30 * math.sin(math.radians(ang)), 8)
        return m
    f["L3_flange_hub_rework"] = [hub]

    def pulley(X, Y, Z):
        R = np.sqrt(X * X + Y * Y)
        # point-in-polygon in (R, Z)
        inside = np.zeros_like(R, dtype=bool)
        n = len(prof)
        for i in range(n):
            r1, z1 = prof[i]
            r2, z2 = prof[(i + 1) % n]
            cond = ((z1 > Z) != (z2 > Z))
            with np.errstate(divide="ignore", invalid="ignore"):
                rint = r1 + (Z - z1) * (r2 - r1) / (z2 - z1 if z2 != z1 else 1e-30)
            inside ^= cond & (R < rint)
        for k in range(6):
            ang = math.radians(60 * k)
            inside &= ~disk(X, Y, 24 * math.cos(ang), 24 * math.sin(ang), 8)
        return inside
    f["L4_v_pulley"] = [pulley]

    def tube(X, Y, Z):
        def ann(d2):
            return (d2 < 64) & (d2 > 25)
        m = (X > 0) & (X <= 60) & ann(Y * Y + Z * Z)
        # arc 1 in XY, centre (60,30)
        th = np.arctan2(Y - 30, X - 60)
        rho = np.sqrt((X - 60) ** 2 + (Y - 30) ** 2)
        m |= (X > 60) & (Y < 30) & ann((rho - 30) ** 2 + Z * Z)
        m |= (Y >= 30) & (Y <= 90) & ann((X - 90) ** 2 + Z * Z)
        rho2 = np.sqrt((Y - 90) ** 2 + (Z - 30) ** 2)
        m |= (Y > 90) & (Z < 30) & ann((rho2 - 30) ** 2 + (X - 90) ** 2)
        m |= (Z >= 30) & (Z < 100) & ann((X - 90) ** 2 + (Y - 120) ** 2)
        return m
    f["L4_bent_tube"] = [tube]

    def transition(X, Y, Z):
        ax, ay = np.abs(X), np.abs(Y)
        base = (ax < 40) & (ay < 40) & (Z > 0) & (Z < 10)
        base &= ~((ax > 28) & (ay > 28) & ((ax - 28) ** 2 + (ay - 28) ** 2 > 144))
        t = (Z - 10) / 40
        hx = 30 - 20 * t
        hy = 10 + 20 * t
        loft = (Z >= 10) & (Z < 50) & (ax < hx) & (ay < hy)
        return (base | loft) & ~disk(X, Y, 0, 0, 12)
    f["L4_loft_on_rounded_base"] = [transition]

    def tray(X, Y, Z):
        m = box(X, Y, Z, -75, 75, -50, 50, 0, 25)
        m &= ~box(X, Y, Z, -72, 72, -47, 47, 3, 99)
        d = comp + 2
        for c in (-(comp / 2 + 1), comp / 2 + 1):
            m |= box(X, Y, Z, c - 1, c + 1, -47, 47, 0, 25)
        for c in (-(comp + 2), 0, comp + 2):
            m &= ~disk(X, Y, c, 0, 10)
        return m
    f["L5_divided_tray"] = [tray]

    def wheel(X, Y, Z):
        R2 = X * X + Y * Y
        m = (R2 < 900) & (Z > 5) & (Z < 25)
        m &= ~((R2 > 729) & (Z > 10) & (Z < 20))
        m |= (R2 < 225) & (Z > 0) & (Z < 30)
        m &= ~(R2 < 36)
        for k in range(6):
            ang = math.radians(60 * k)
            m &= ~disk(X, Y, 20 * math.cos(ang), 20 * math.sin(ang), 8)
        return m
    f["L5_idler_wheel"] = [wheel]

    f["L5_pillow_block_set"] = [
        lambda X, Y, Z: box(X, Y, Z, -60, 60, -30, 30, 0, 10) & ~disk(X, Y, 20, 0, 8) & ~disk(X, Y, -20, 0, 8)
        & ~disk(X, Y, 50, 20, 10) & ~disk(X, Y, -50, 20, 10) & ~disk(X, Y, 50, -20, 10) & ~disk(X, Y, -50, -20, 10),
        lambda X, Y, Z: box(X, Y, Z, -30, 30, -15, 15, 10, 50) & ~(X * X + (Z - 30) ** 2 < 12.5 ** 2)
        & ~disk(X, Y, 20, 0, 8) & ~disk(X, Y, -20, 0, 8),
        lambda X, Y, Z: (X * X + (Z - 30) ** 2 < 144) & (np.abs(Y) < 40),
    ]

    worst = 0
    for name, t in tasks.items():
        for body, fn in zip(t["bodies"], f[name]):
            lo, hi = body["bbox"]
            lo = [v - 0.5 for v in lo]
            hi = [v + 0.5 for v in hi]
            g = integrate(fn, lo, hi)
            err = (g - body["volume"]) / body["volume"] * 100
            worst = max(worst, abs(err))
            print(f"{name:28s} {body['name']:10s} exact {body['volume']:14.4f} grid {g:14.2f} diff {err:+.3f}%")
    print("worst diff %.3f%%" % worst)


if __name__ == "__main__":
    for name, t in tasks.items():
        for b in t["bodies"]:
            print(f"{name:28s} {b['name']:10s} V={b['volume']:14.4f} bbox={b['bbox']}")
    with open(os.path.join(HERE, "tasks.json"), "w", newline="\n") as fh:
        json.dump(tasks, fh, indent=2)
        fh.write("\n")
    if "--check" in sys.argv:
        check()
