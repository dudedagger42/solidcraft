# Repro: History bar hover stalls on a large STEP assembly

`shaped_assembly.step` is synthetic and public-safe: every part is a box drilled with round through-holes, every name is `P<n>`, and nothing else is in it. Its structure mimics a real 3,222-body printer assembly: the same product tree (787 products, 2,832 occurrences, 8 levels deep), the same solids per part, and holes standing in for its curved faces, so that it meshes to about the same size (8.42 M display triangles against the real 8.47 M).

- `step_shape2.py private.step --tree tree.json`: reads a STEP's structure only (products renamed P0, P1… in order of first use, counts of solids, faces and curved faces per part; no names, dimensions or metadata).
- `gen_assembly2.py --tree tree.json --hole-share 0.22 --max-holes 2000 --out shaped_assembly.step`: rebuilds it from drilled boxes. `--hole-share` scales the holes per curved face to match a triangle count.
