//! A design can keep its own orientation (Navigator: Set as Front / Top): the world axis its front and
//! its top face, and each named view records the orientation it was saved in. Older designs have neither;
//! they turn by the up axis preference and their views come back as saved. The last release warns about
//! the newer format number, opens the design without its own orientation and keeps the field when an
//! updated design is saved there.
