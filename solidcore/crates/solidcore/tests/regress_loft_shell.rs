//! Probe for SolidCraft's REGRESSION-LOFT-SHELL: a plain circle-to-square loft shelled 2 mm
//! open at the circle builds on solidcore a445bdaac (rc.3) and fails on c0d3cc0124 (the vendored
//! commit) with SolidCraft's fixes. Bisect helper; prints `REGRESS:` lines.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use solidcore::curve::curve::Curve;
use solidcore::curve::ellipse::Ellipse;
use solidcore::curve::line::Line;
use solidcore::geom::axis2_placement::Axis2Placement;
use solidcore::geom::extent1d::Extent1d;
use solidcore::geom::vector3d::{Point3, Vec3};
use solidcore::kernel::{Kernel, KernelOptions};
use solidcore::offset::offset_executive::p2::{shell_brep_simple, OffsetOptions, ShellOptions};
use solidcore::query::api_queries::face_bounding_box;
use solidcore::sweep::api_surfaces::create_skin_primitive;
use solidcore::topology::brep::p1 as brep_p1;
use solidcore::topology::brep::p2::is_manifold_solid;

/// SolidCraft's fixes (`crates/smlib/src/core.rs` `fixes()` on main c71236a40).
#[allow(clippy::too_many_lines)]
fn solidcraft_fixes(k: &mut Kernel) {
    k.fixes.fillet_refuse_rails_leaving_faces = true;
    k.fixes.fillet_refuse_face_consuming_blends = true;
    k.fixes.fillet_vertex_error_diagnostics = true;
    k.fixes.fillet_chamfer_cone_on_arc = true;
    k.fixes.offset_excess_faces_after_merge = true;
    k.fixes.offset_clip_right_side_out = true;
    k.fixes.offset_extended_edges_mated = true;
    k.fixes.keep_operand_uv_trims = true;
    k.fixes.proto_vertex_miss_is_false = true;
    k.fixes.check_loop_vertex_tolerance = true;
    k.fixes.check_loop_projected_seam_ends = true;
    k.fixes.oscillating_skips_failed_guesses = true;
    k.fixes.refine_cc_range_ends = true;
    k.fixes.flat_corners_test_bits = true;
    k.fixes.cylinder_rebuild_mean_radius = true;
    k.fixes.revolve_analytic_profile_faces = true;
    k.fixes.taper_tangent_corner_planes = true;
    k.fixes.revolve_nested_shells_as_cavities = true;
    k.fixes.revolve_holed_profile_void = true;
    k.fixes.join_touching_loops = true;
    k.fixes.offset_resolve_crossing_loops = true;
    k.fixes.offset_analytic_mate_edges = true;
    k.fixes.nurb_cylinder_inside_out_frame = true;
    k.fixes.local_operation_refuses_loose_tangent_edges = true;
    k.fixes.torus_major_radius_from_origin = true;
    k.fixes.torus_iso_circle_axis_from_midpoint = true;
    k.fixes.internal_pole_row_through_knot = true;
    k.fixes.extrusion_origin_scaled_zero = true;
    k.fixes.transform_scales_each_tolerance = true;
    k.fixes.transform_model_size_from_body_box = true;
    k.fixes.offset_collapsed_rounds_mated = true;
    k.fixes.uv_trim_corner_end_points_only = true;
    k.fixes.bspline_trim_preserves_uv_spans = true;
    k.fixes.proto_edge_self_loop_on_vertex_degenerate = true;
    k.fixes.slice_coincident_faces_sheet_side = true;
    k.fixes.boolean_outer_shell_first = true;
    k.fixes.cover_refuses_folded_continuation = true;
    k.fixes.boolean_vertex_by_best_crossing = true;
    k.fixes.boolean_cleanup_removes_slits = true;
    k.fixes.classify_edge_interval = true;
    k.fixes.ellipse_trim_seam_lines_span_cone_height = true;
    k.fixes.degenerate_proto_edge_in_one_vertex = true;
    k.fixes.new_edge_vertex_tolerance_covers_gap = true;
    k.fixes.torus_signed_gen_curve_interval = true;
    k.fixes.point_solve_keeps_triangle_guess = true;
    k.fixes.imprint_extends_edge_to_proto_vertex = true;
    k.fixes.fillet_point_step_per_point = true;
    k.fixes.analytic_copy_keeps_parameterization = true;
    k.fixes.offset_extension_from_base = true;
    k.fixes.fillet_cleanup_keeps_target_faces = true;
    k.fixes.fillet_nx1_tangent_side_edge = true;
    k.fixes.offset_stitch_at_body_tolerance = true;
    k.fixes.curve_contacts_at_curve_ends = true;
    k.fixes.shell_empty_cavity_fails = true;
    k.fixes.ssi_tangent_contact_points = true;
    k.fixes.ssi_trace_tangent_curves_once = true;
    k.fixes.ssi_singular_start_inside_direction = true;
    k.fixes.found_torus_sphere_keep_tested_nurb = true;
    k.fixes.local_operation_box_holds_new_surfaces = true;
    k.fixes.shell_edges_on_analytics_exact = true;
    k.fixes.ssi_pole_point_keeps_its_tangent = true;
    k.fixes.fillet_patch_keeps_fillet_edges = true;
    k.fixes.boolean_splits_definition_across_split = true;
    k.fixes.fillet_refuse_overlapping_bands = true;
    k.fixes.ssi_tangent_curve_tracer = true;
    k.fixes.torus_check_spindle_apex = true;
    k.fixes.local_operation_step_walls = true;
    k.fixes.edge_check_orders_tangent_faces = true;
    k.fixes.thicken_planar_side_walls = true;
    k.fixes.fillet_knife_edge_rounds = true;
    k.fixes.revolution_round_offset_collapses = true;
    k.fixes.offset_point_vertex_caps = true;
    k.fixes.proto_edge_splits_at_captured_vertices = true;
    k.fixes.local_operation_closes_walls_to_nothing = true;
    k.fixes.imprint_reuses_coincident_face_edge = true;
    k.fixes.boolean_new_face_closure_both_sides = true;
    k.fixes.linear_sweep_arc_is_cylinder = true;
    k.fixes.loops_go_on_round_the_vertex = true;
    k.fixes.revolve_splits_curves_on_axis = true;
    k.fixes.cover_wraps_across_closed_seam = true;
    k.fixes.fillet_trace_reaches_rails = true;
    k.fixes.delete_vertex_joins_seam_iso_curve = true;
    k.fixes.box_face_failures_propagate = true;
    k.fixes.fillet_rail_end_on_continued_surface = true;
    k.fixes.fillet_removal_takes_ring_fillets = true;
    k.fixes.fillet_trim_inside_by_best_dot = true;
    k.fixes.fillet_half_degree_crease = true;
    k.fixes.curve_loop_normal_from_area = true;
    k.fixes.fillet_cliff_overflow = true;
    k.fixes.offset_extension_as_itself = true;
    k.fixes.fillet_mitre_concave_face_corners = true;
    k.fixes.fillet_mitre_gap_patch = true;
    k.fixes.good_drop_result_distances_in_order = true;
    k.fixes.face_tight_box_searches_trim = true;
    k.fixes.ssi_low_angle_start_on_probation = true;
    k.fixes.fillet_variable_span_follows_fillet = true;
    k.fixes.simple_coincidence_crossing_is_point = true;
    k.fixes.planar_path_sweep_statuses = true;
    k.fixes.planar_path_profile_placement_statuses = true;
    k.fixes.fillet_trim_status = true;
    k.fixes.brep_edit_statuses_pass_on = true;
    k.fixes.fillet_run_on_bspline_trims = true;
    k.fixes.query_statuses_pass_on = true;
    k.fixes.sweep_statuses_pass_on = true;
    k.fixes.ssi_proto_topology_statuses_pass_on = true;
    k.fixes.analytic_inversion_answers_apart = true;
    k.fixes.fillet_statuses_pass_on = true;
    k.fixes.fillet_corner_per_vertex_fan = true;
    k.fixes.fillet_rail_trim_at_near_knot = true;
    k.fixes.offset_keep_cavity_regions = true;
    k.fixes.ellipse_step_interval_checks = true;
    k.fixes.corner_patch_face_side_twist = true;
    k.fixes.boolean_union_refuses_solid_leak = true;
    k.fixes.curve_loop_start_after_previous_end = true;
    k.fixes.loop_use_reports_open_gaps = true;
    k.fixes.face_points_span_domain = true;
    k.fixes.drop_curve_v_domain_degeneracy = true;
    k.fixes.mirror_curve_keeps_euclidean_control_points = true;
    k.fixes.dupin_bottom_hyperbola_axis = true;
    k.fixes.offset_swap_uv_extended_distance = true;
    k.fixes.validation_does_not_make_uv_trims = true;
    k.fixes.proto_edge_split_upper_keeps_the_old_max = true;
    k.fixes.point_on_start_point_squared_tolerance = true;
    k.fixes.bspline_trim_keeps_rational_end_spans = true;
    k.fixes.fillet_exact_round_classes = true;
    k.fixes.check_seam_crossing_unclosed = true;
    k.fixes.ascii_doubles_round_trip = true;
    k.fixes.offset_edges_keep_source_tolerance = true;
    k.fixes.surface_continuity_checks_last_v_knot = true;
    k.fixes.volume_lift_third_derivative_terms = true;
    k.fixes.curve_bounded_surface_read_uv_type_and_implicit_outer = true;
    k.fixes.volume_representational_end_continuities = true;
    k.fixes.offset_open_edge_second_trim_point = true;
    k.fixes.close_loop_relates_only_coincident_edges = true;
    k.fixes.ssi_split_curves_at_singular_points = true;
    k.fixes.surface_parallel_to_vector_checks = true;
    k.fixes.geometric_g2_continuity = true;
    k.fixes.degen_param_tiebreak_uses_actual_param = true;
    k.fixes.local_surface_intersect_guards_max_boundary_retry = true;
    k.fixes.curve_sweep_closed_path_joins_ends = true;
    k.fixes.validate_tolerances_fixes_reported_gaps = true;
    k.fixes.edgeuse_convexity_propagates_step_off_failure = true;
    k.fixes.cone_create_canonical_preserves_inside_out_and_checks_height = true;
    k.fixes.point_class_gap_3d_propagates_no_drop_error = true;
    k.fixes.cone_iso_circle_at_axial_height = true;
    k.fixes.fillet_overflow_trim_at_creases = true;
    k.fixes.cone_unbounded_parameterized_as_nurb = true;
    k.fixes.fillet_round_as_definition = true;
    k.fixes.fillet_overflow_crease_along_edge = true;
    k.fixes.keep_exact_pcurves = true;
    k.fixes.face_swap_uv_transposes_trims = true;
    k.fixes.closed_smooth_pole_tangent_limit = true;
    k.fixes.fillet_chamfer_corner = true;
    k.fixes.brep_ray_classifies_current_trims = true;
}

fn loft_circle_to_square(k: &mut Kernel) -> solidcore::topology::Brep {
    let mut curves: Vec<Curve> = Vec::new();
    // Four quarter arcs of the circle r 20 at z 0, each starting at 45 + 90 i degrees, ccw.
    for i in 0..4 {
        let a0 = (45.0 + 90.0 * f64::from(i)).to_radians();
        let x0 = Vec3::new(a0.cos(), a0.sin(), 0.0);
        let y0 = Vec3::new(-a0.sin(), a0.cos(), 0.0);
        let frame = Axis2Placement::new(Point3::new(0.0, 0.0, 0.0), x0, y0);
        let degrees = Extent1d::from_min_max(0.0, 90.0);
        let c = Ellipse::create_canonical_circle(k, &frame, 20.0, Some(&degrees), None).unwrap();
        curves.push(Curve::from(c));
    }
    // The square +-15 at z 40, ccw from (15, 15): side i pairs with arc i.
    let sq = [[15.0, 15.0], [-15.0, 15.0], [-15.0, -15.0], [15.0, -15.0]];
    for i in 0..4 {
        let a = sq[i];
        let b = sq[(i + 1) % 4];
        let l = Line::create_line_segment(3, Vec3::new(a[0], a[1], 40.0), Vec3::new(b[0], b[1], 40.0), None).unwrap();
        curves.push(Curve::from(l));
    }
    let refs: Vec<Option<&Curve>> = curves.iter().map(Some).collect();
    create_skin_primitive(k, &[4, 4], &refs, 1, true).unwrap().brep
}

#[test]
fn a_circle_to_square_loft_shells_open_at_the_circle() {
    let mut k = Kernel::new(&KernelOptions::default());
    solidcraft_fixes(&mut k);
    let b = loft_circle_to_square(&mut k);
    let faces = brep_p1::faces(&b, None).unwrap();
    println!("REGRESS: loft faces={} manifold={}", faces.len(), is_manifold_solid(&b).unwrap());
    assert!(is_manifold_solid(&b).unwrap());
    for &f in &faces {
        let bx = face_bounding_box(&k, &b, f, false).unwrap();
        println!("REGRESS: face {:?} box min {:?} max {:?}", f, bx.min(), bx.max());
    }
    let cap = faces
        .iter()
        .copied()
        .find(|&f| {
            let bx = face_bounding_box(&k, &b, f, false).unwrap();
            bx.max().z < 1e-3 && bx.min().z > -1e-3
        })
        .expect("a cap at z 0");
    let options = ShellOptions {
        offset: OffsetOptions { distance: -2.0, extend_convex_edges: true, merge_results: true },
        create_offset_solid: true,
        merge_sheet_side_walls: true,
    };
    match shell_brep_simple(&mut k, &b, options, &[cap]) {
        Ok(s) => {
            let n = brep_p1::faces(&s, None).unwrap().len();
            let m = is_manifold_solid(&s).unwrap();
            println!("REGRESS: shell ok faces={n} manifold={m}");
            assert!(m, "the shell is not a manifold solid ({n} faces)");
        }
        Err(e) => {
            println!("REGRESS: shell err {e}");
            panic!("shell refused: {e}");
        }
    }
}
