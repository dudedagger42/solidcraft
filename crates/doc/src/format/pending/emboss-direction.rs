//! An Emboss saves its Direction (`direction`: `normal_to_face`, the profiles laid on the target
//! face keeping their size and raised along its normal) and its Stop at face edge flag
//! (`stop_at_edge`: trimmed at the edge of the target face), each only when it is not the
//! default, and its extra Target Faces (`more_faces`: points on more faces of the target body, in
//! the same frame as `face`'s and named after it), only when there are some. An older design gets
//! Normal to sketch, no stop and one face: the emboss it always built. The last release warns
//! about the newer format, opens the design and builds such an Emboss Normal to sketch without
//! the stop on its first face only, keeping the unknown fields when it saves.
