//! Widening pinched profiles (#9216): where a region's outline touches itself, the touching
//! loops are offset away from the material by δ, so the neck they pinch to gets width 2δ. The
//! region stays one region.

use solidcraft_geom::prelude::*;
use solidcore_record::{Loop2, Region2, Seg2, Vec2};

use crate::profiles::ENDPOINT_JOIN_TOL;

/// δ for a region: max(ENDPOINT_JOIN_TOL, 1e-6 × the region's bbox diagonal).
pub fn pinch_width(_r: &Region2) -> f64 {
    todo!()
}

/// Regions whose loops touch each other are returned with each touching loop offset as a whole
/// by δ away from the material (outer loop outward, hole inward). Untouched regions and loops
/// come back bit for bit the same.
pub fn widen_pinches(_regions: Vec<Region2>) -> Vec<Region2> {
    todo!()
}

/// Minimum distance between two loops, searched up to `cutoff` (larger values may be returned
/// when nothing lies within it).
fn loop_gap(_a: &Loop2, _b: &Loop2, _cutoff: f64) -> f64 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sq(s: f64) -> Loop2 {
        Loop2::polygon(&[Vec2::new(0.0, 0.0), Vec2::new(s, 0.0), Vec2::new(s, s), Vec2::new(0.0, s)])
    }

    fn rect(w: f64, h: f64) -> Loop2 {
        Loop2::polygon(&[Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, h), Vec2::new(0.0, h)])
    }

    fn gap(a: &Loop2, b: &Loop2) -> f64 {
        loop_gap(a, b, f64::INFINITY)
    }

    /// Radius of a loop made of one arc.
    fn radius(l: &Loop2) -> f64 {
        match l.segs.first() {
            Some(Seg2::Arc { radius, .. }) => *radius,
            _ => f64::NAN,
        }
    }

    fn centre(l: &Loop2) -> Vec2 {
        match l.segs.first() {
            Some(Seg2::Arc { center, .. }) => *center,
            _ => Vec2::new(f64::NAN, f64::NAN),
        }
    }

    /// Axis-aligned extent of a loop's segment ends (exact for polygons).
    fn extent(l: &Loop2) -> (Vec2, Vec2) {
        let mut lo = Vec2::new(f64::INFINITY, f64::INFINITY);
        let mut hi = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for s in &l.segs {
            for p in [s.start(), s.end()] {
                lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
                hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
            }
        }
        (lo, hi)
    }

    fn area(r: &Region2) -> f64 {
        r.outer.signed_area().abs() - r.holes.iter().map(|h| h.signed_area().abs()).sum::<f64>()
    }

    #[test]
    fn square_with_tangent_hole_gets_a_gap() {
        let outer = sq(10.0);
        let hole = Loop2::circle(Vec2::new(5.0, 2.5), 2.5);
        let r = Region2 { outer, holes: vec![hole] };
        let d = pinch_width(&r);
        let out = widen_pinches(vec![r]);
        assert_eq!(out.len(), 1);
        let o = &out[0];
        assert_eq!(o.holes.len(), 1);
        assert!((radius(&o.holes[0]) - (2.5 - d)).abs() < 1e-12);
        let (lo, hi) = extent(&o.outer);
        assert!((hi.x - lo.x - (10.0 + 2.0 * d)).abs() < 1e-12);
        assert!((gap(&o.outer, &o.holes[0]) - 2.0 * d).abs() < 1e-12);
        assert!((area(o) - ((10.0 + 2.0 * d).powi(2) - PI * (2.5 - d).powi(2))).abs() < 1e-9);
    }

    /// The hole as two arcs split at (5, 0), the bottom edge in two pieces; with `split_off`
    /// the bottom pieces meet at (5, 1e-9) instead of (5, 0).
    fn split_case(split_off: bool) -> Region2 {
        let mid = if split_off { Vec2::new(5.0, 1e-9) } else { Vec2::new(5.0, 0.0) };
        let outer = Loop2 {
            segs: vec![
                Seg2::Line { a: Vec2::new(0.0, 0.0), b: mid },
                Seg2::Line { a: mid, b: Vec2::new(10.0, 0.0) },
                Seg2::Line { a: Vec2::new(10.0, 0.0), b: Vec2::new(10.0, 10.0) },
                Seg2::Line { a: Vec2::new(10.0, 10.0), b: Vec2::new(0.0, 10.0) },
                Seg2::Line { a: Vec2::new(0.0, 10.0), b: Vec2::new(0.0, 0.0) },
            ],
        };
        let c = Vec2::new(5.0, 2.5);
        let hole = Loop2 {
            segs: vec![
                Seg2::Arc { center: c, radius: 2.5, start: -PI / 2.0, sweep: PI },
                Seg2::Arc { center: c, radius: 2.5, start: PI / 2.0, sweep: PI },
            ],
        };
        Region2 { outer, holes: vec![hole] }
    }

    #[test]
    fn split_arcs_and_split_edge_get_a_gap() {
        let r = split_case(false);
        let d = pinch_width(&r);
        let out = widen_pinches(vec![r]);
        assert_eq!(out.len(), 1);
        let o = &out[0];
        assert!((gap(&o.outer, &o.holes[0]) - 2.0 * d).abs() < 1e-12);
        assert!((area(o) - ((10.0 + 2.0 * d).powi(2) - PI * (2.5 - d).powi(2))).abs() < 1e-9);
    }

    #[test]
    fn split_point_off_the_tangent_by_1e9_still_gets_a_gap() {
        // The input itself is 1e-9 off tangent, so the gap is 2δ to that accuracy.
        let r = split_case(true);
        let d = pinch_width(&r);
        let out = widen_pinches(vec![r]);
        assert_eq!(out.len(), 1);
        let o = &out[0];
        assert!((gap(&o.outer, &o.holes[0]) - 2.0 * d).abs() < 1e-8);
        assert!((area(o) - ((10.0 + 2.0 * d).powi(2) - PI * (2.5 - d).powi(2))).abs() < 1e-7);
    }

    #[test]
    fn two_tangent_holes_separate() {
        let outer = rect(20.0, 10.0);
        let r = Region2 {
            outer: outer.clone(),
            holes: vec![Loop2::circle(Vec2::new(8.0, 5.0), 2.0), Loop2::circle(Vec2::new(12.0, 5.0), 2.0)],
        };
        let d = pinch_width(&r);
        let out = widen_pinches(vec![r]);
        assert_eq!(out.len(), 1);
        let o = &out[0];
        assert_eq!(o.outer, outer);
        assert_eq!(o.holes.len(), 2);
        assert!((radius(&o.holes[0]) - (2.0 - d)).abs() < 1e-12);
        assert!((radius(&o.holes[1]) - (2.0 - d)).abs() < 1e-12);
        assert_eq!(centre(&o.holes[0]), Vec2::new(8.0, 5.0));
        assert_eq!(centre(&o.holes[1]), Vec2::new(12.0, 5.0));
    }

    #[test]
    fn untouched_region_is_bit_for_bit_the_same() {
        let r = Region2 { outer: sq(10.0), holes: vec![Loop2::circle(Vec2::new(5.0, 5.0), 2.0)] };
        assert_eq!(widen_pinches(vec![r.clone()]), vec![r]);
    }

    #[test]
    fn self_touching_loop_is_left_alone() {
        // Two squares sharing the corner (4, 4), drawn as one loop that visits it twice.
        let pts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(8.0, 4.0),
            Vec2::new(8.0, 8.0),
            Vec2::new(4.0, 8.0),
            Vec2::new(4.0, 4.0),
            Vec2::new(0.0, 4.0),
        ];
        let r = Region2 { outer: Loop2::polygon(&pts), holes: vec![] };
        assert_eq!(widen_pinches(vec![r.clone()]), vec![r]);
    }

    #[test]
    fn figure_eight_is_left_alone() {
        let a = Region2 { outer: Loop2::circle(Vec2::new(0.0, 0.0), 1.0), holes: vec![] };
        let b = Region2 { outer: Loop2::circle(Vec2::new(2.0, 0.0), 1.0), holes: vec![] };
        let input = vec![a, b];
        assert_eq!(widen_pinches(input.clone()), input);
    }

    #[test]
    fn pinch_width_scales_with_region_size() {
        let big = Region2 { outer: rect(72.0, 96.0), holes: vec![] };
        assert!((pinch_width(&big) - 1.2e-4).abs() < 1e-12);
        let small = Region2 { outer: rect(0.6, 0.8), holes: vec![] };
        assert!((pinch_width(&small) - 1e-4).abs() < 1e-12);
    }
}
