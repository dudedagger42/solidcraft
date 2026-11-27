//! A component that starts a timeline of its own (a part, DM-4.4) saves `timeline: true` on its
//! component entry. Older designs carry no such flag and open as one part, the design itself,
//! which is the flag's default. The last release warns about the newer format, opens the design
//! with one timeline and the same geometry, and keeps the flag when saving.
