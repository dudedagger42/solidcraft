//! A design can have its own FRONT and TOP: `view_frame` (set by Set as Front / Top on the axes
//! navigator), the frame a saved render camera's angles are in, and the frame each named view's
//! angles are in (`named_views[].frame`). Older designs have none: their views, Home and the orbit
//! follow the up axis preference, as before, and a named view with no frame turns into the design's
//! own frame when it is applied. The last release asks for an update instead of meeting it: it opens
//! a design that has these with a newer-format warning, ignores the frames and keeps what it can read.
