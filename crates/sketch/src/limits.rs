//! The largest sketch SolidCraft builds (#10874). [`DEFAULT_POINTS`] is the limit sketches have
//! always had; the app may raise it (Preferences › Sketch › Largest sketch), never past
//! [`CEILING_POINTS`]. Every place that adds points, curves or constraints asks [`max_points`],
//! [`max_curves`] or [`max_constraints`] for the limit in force. Reading a sketch that is
//! already in a design checks the ceiling instead ([`crate::CEILING_POINTS`] and the others),
//! so a design made with a raised limit opens anywhere.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The most points (and curves) a sketch takes by default: the limit sketches have always had.
pub const DEFAULT_POINTS: usize = 20_000;
/// The highest the limit may be raised to; also what a sketch read from a design may hold.
pub const CEILING_POINTS: usize = 200_000;

static PREFERRED: AtomicUsize = AtomicUsize::new(DEFAULT_POINTS);

/// Use this limit on points (and curves) from now on; constraints may be twice as many.
/// Refused, changing nothing, outside [`DEFAULT_POINTS`]..=[`CEILING_POINTS`].
pub fn set_max_points(points: usize) -> Result<(), String> {
    if !(DEFAULT_POINTS..=CEILING_POINTS).contains(&points) {
        return Err(format!("the largest sketch must be between {DEFAULT_POINTS} and {CEILING_POINTS} points, not {points}"));
    }
    PREFERRED.store(points, Ordering::Relaxed);
    Ok(())
}

/// The most points a sketch takes now.
pub fn max_points() -> usize {
    PREFERRED.load(Ordering::Relaxed).clamp(DEFAULT_POINTS, CEILING_POINTS)
}

/// The most curves a sketch takes now: curves need points, so this follows the point limit.
pub fn max_curves() -> usize {
    max_points()
}

/// The most constraints a sketch takes now: twice the point limit, as by default.
pub fn max_constraints() -> usize {
    max_points().saturating_mul(2)
}

static WIDEN_PINCHES: AtomicBool = AtomicBool::new(true);

/// Where a profile's loops touch at a point (a pinch, #9216), features widen the neck by about
/// 0.1 µm so the solid is manifold; off keeps the exact touching outline (before #9216).
pub fn set_widen_pinched_profiles(on: bool) {
    WIDEN_PINCHES.store(on, Ordering::Relaxed);
}

/// Whether pinched profiles widen now (on by default; Preferences › Sketch).
pub fn widen_pinched_profiles() -> bool {
    WIDEN_PINCHES.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #10874: the limit rises only within its range; a refused value leaves it unchanged.
    #[test]
    fn the_sketch_limit_rises_within_its_range() {
        assert_eq!(max_points(), DEFAULT_POINTS);
        assert!(set_max_points(DEFAULT_POINTS - 1).is_err());
        assert!(set_max_points(CEILING_POINTS + 1).is_err());
        assert_eq!(max_points(), DEFAULT_POINTS, "a refused value changes nothing");
        assert_eq!((max_curves(), max_constraints()), (DEFAULT_POINTS, 2 * DEFAULT_POINTS));
    }

    /// #9216: pinched profiles widen by default; the preference turns the widening off and back on.
    #[test]
    fn widening_pinched_profiles_defaults_on_and_round_trips() {
        assert!(widen_pinched_profiles());
        set_widen_pinched_profiles(false);
        assert!(!widen_pinched_profiles());
        set_widen_pinched_profiles(true);
        assert!(widen_pinched_profiles());
    }
}
