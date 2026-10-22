# Slow cut: a lofted air path through a 3D-printer duct

A `kernel-perf` test case for SolidCraft. Two STEP files hold the operands exactly as they were before the cut:

- `fan_mount_v2.step`: the target, a fan mount with its lofted ducts joined. 22 faces (8 cylinders, 9 planes, 5 B-spline), 21,947.7 mm³.
- `r_air_v3.step`: the tool, a lofted air path. 4 faces (2 planes, 2 B-spline), 6,474.1 mm³.

Run it with:

```
solidcraft-cli run cut.json --timings --profile
```

On 0.19.0 the cut takes about 171 s, nearly all of it `boolean_ms`. The result is 15,769.27 mm³, 272 kernel faces and one shell.

Shared by the part's author for SolidCraft bug reports.
