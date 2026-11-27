//! An Emboss saves its Direction (`direction`: `normal_to_face`, the profiles laid on the target
//! face keeping their size and raised along its normal) and its Stop at face edge flag
//! (`stop_at_edge`: trimmed at the edge of the target face), each only when it is not the default.
//! An older design gets Normal to sketch and no stop: the emboss it always built. The last release
//! warns about the newer format, opens the design and builds such an Emboss Normal to sketch
//! without the stop, keeping the unknown fields when it saves.
