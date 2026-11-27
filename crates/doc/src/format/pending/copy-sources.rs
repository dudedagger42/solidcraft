//! A part copied with its history from another design (Insert Design as Copy, DM-4.4) saves a
//! `sources` entry under its component id: the source file's path, its SHA-256 as copied and the
//! copied design's JSON. Older designs have no copies, which is the field's default. The last
//! release warns about the newer format, opens the design with the copied parts as ordinary
//! components, and keeps the entries when saving.
