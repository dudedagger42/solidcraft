//! Navigation drawing definitions. Camera projection and picking remain in the UI widgets.
use egui::{Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};
use solidcore_record::Vec3;
use solidcraft_engine::geom::Ring;
use solidcraft_theme::Tokens;
pub(crate) struct Facet {
    pub(crate) direction: Vec3,
    pub(crate) points: Ring<Pos2>,
    pub(crate) outer_edges: Vec<bool>,
    pub(crate) centre: Pos2,
    pub(crate) label: String,
    pub(crate) text_axes: Option<(egui::Vec2, egui::Vec2)>,
    pub(crate) depth: f64,
}
// Private marker for label meshes with atlas-pixel UVs. The output hook replaces
// it with the font texture before any renderer sees the frame.
pub(crate) const LABEL_PIXELS: egui::TextureId = egui::TextureId::User(u64::from_le_bytes(*b"cubetext"));

struct LabelAtlas;

impl egui::plugin::Plugin for LabelAtlas {
    fn debug_name(&self) -> &'static str {
        "navigation cube label atlas"
    }

    fn output_hook(&mut self, ctx: &egui::Context, output: &mut egui::FullOutput) {
        let size = ctx.fonts(|f| f.font_image_size());
        let normalize = vec2(1.0 / size[0].max(1) as f32, 1.0 / size[1].max(1) as f32);
        for shape in &mut output.shapes {
            if let Shape::Mesh(mesh) = &mut shape.shape
                && mesh.texture_id == LABEL_PIXELS
            {
                let mesh = std::sync::Arc::make_mut(mesh);
                mesh.texture_id = egui::TextureId::default();
                for v in &mut mesh.vertices {
                    v.uv = (v.uv.to_vec2() * normalize).to_pos2();
                }
            }
        }
    }
}

/// Warp the font mesh into the projected face, so labels turn with the model.
pub(crate) fn face_text(painter: &egui::Painter, face: &Facet, ink: Color32, font_size: f32) {
    let Some((r, down)) = face.text_axes else { return };
    let opacity = ((face.depth - 0.18) / 0.17).clamp(0.0, 1.0) as f32;
    if opacity <= 0.0 {
        return;
    }
    let ink = ink.gamma_multiply(opacity);
    let mut galley = painter.layout_no_wrap(face.label.clone(), FontId::proportional(font_size), ink);
    // Long names (other languages) shrink to 70% of the face's width along the text.
    let rows = down * 1.35;
    let across = r.x * rows.y - r.y * rows.x;
    let half_width =
        face.points.iter().map(|p| ((p.x - face.centre.x) * rows.y - (p.y - face.centre.y) * rows.x) / across).fold(0.0, |w, x| x.abs().max(w));
    if across.abs() > 1e-6 && galley.rect.width() > 1.4 * half_width {
        galley = painter.layout_no_wrap(face.label.clone(), FontId::proportional(font_size * 1.4 * half_width / galley.rect.width()), ink);
    }
    let centre = galley.rect.center();
    // Keep atlas-pixel UVs until the frame is complete: another label or widget
    // can grow the atlas after this mesh is created, invalidating early UVs.
    painter.ctx().add_plugin(LabelAtlas);
    let text = Shape::Text(egui::epaint::TextShape::new(Pos2::ZERO, galley, ink));
    let atlas = painter.ctx().fonts(|f| f.font_image_size());
    let primitives = painter.ctx().tessellate(vec![egui::epaint::ClippedShape::new(Rect::EVERYTHING, text)], painter.ctx().pixels_per_point());
    for primitive in primitives {
        if let egui::epaint::Primitive::Mesh(mut mesh) = primitive.primitive {
            mesh.texture_id = LABEL_PIXELS;
            for v in &mut mesh.vertices {
                v.uv = pos2(v.uv.x * atlas[0] as f32, v.uv.y * atlas[1] as f32);
                let local = v.pos - centre;
                v.pos = face.centre + r * local.x + down * (local.y * 1.35);
            }
            painter.add(Shape::mesh(mesh));
        }
    }
}

pub(crate) fn paint_cube(painter: &egui::Painter, faces: &[Facet], hovered: Option<usize>, t: Tokens, labels: bool, scale: f32) {
    // A shared, opaque triangle mesh has no antialiasing fringes between facets.
    // Per-polygon fringes and miter joins overlap as a face becomes edge-on.
    let mut fill_mesh = egui::epaint::Mesh::default();
    let mut edges: Vec<(Pos2, Pos2, usize, bool)> = Vec::new();
    let hovered_direction = hovered.and_then(|i| faces.get(i)).map(|f| f.direction);
    for face in faces {
        let base = t.cube_face;
        let shade = (0.72 + 0.28 * face.depth.min(1.0)) as f32;
        let highlighted = hovered_direction == Some(face.direction);
        let fill = if highlighted {
            t.cube_hover
        } else {
            Color32::from_rgb((f32::from(base.r()) * shade) as u8, (f32::from(base.g()) * shade) as u8, (f32::from(base.b()) * shade) as u8)
        };
        let centre = fill_mesh.vertices.len() as u32;
        fill_mesh.colored_vertex(face.centre, fill);
        for &p in &face.points {
            fill_mesh.colored_vertex(p, fill);
        }
        for (j, (&a, &b)) in face.points.edges().enumerate() {
            fill_mesh.add_triangle(centre, centre + 1 + j as u32, centre + 1 + ((j + 1) % face.points.len()) as u32);
            // Coplanar picking regions have no visible seams. Only actual cube
            // edges and the hovered target's boundary receive an outline.
            let outer = face.outer_edges.get(j).copied().unwrap_or(false);
            if !highlighted && !outer {
                continue;
            }
            if let Some(edge) = edges
                .iter_mut()
                .find(|(p, q, _, _)| (p.distance_sq(a) < 1e-6 && q.distance_sq(b) < 1e-6) || (p.distance_sq(b) < 1e-6 && q.distance_sq(a) < 1e-6))
            {
                edge.2 += usize::from(highlighted);
                edge.3 |= outer;
            } else {
                edges.push((a, b, usize::from(highlighted), outer));
            }
        }
    }
    painter.add(Shape::mesh(fill_mesh));
    for (a, b, highlights, outer) in edges {
        // Two highlighted coplanar patches share an internal subdivision, not
        // a target boundary. Actual cube edges remain visible across faces.
        if !outer && highlights != 1 {
            continue;
        }
        let highlighted = highlights > 0;
        painter.line_segment([a, b], Stroke::new(1.0, if highlighted { t.accent } else { t.cube_border }));
    }
    if labels {
        for face in faces {
            face_text(painter, face, t.cube_text, t.font_heading * scale);
        }
    }
}
/// A wide triangle around `at` whose tip points along `direction` (its length is the scale).
pub(crate) fn arrow(painter: &egui::Painter, at: Pos2, direction: egui::Vec2, hover: bool, t: Tokens) {
    let side = vec2(-direction.y, direction.x);
    painter.add(Shape::convex_polygon(
        vec![at + direction * 8.0, at - direction * 6.0 + side * 11.0, at - direction * 6.0 - side * 11.0],
        if hover { t.accent_soft } else { t.cube_face },
        Stroke::new(1.0, if hover { t.accent } else { t.cube_border }),
    ));
}
/// Radius of the curved roll arrows' circle, in design units: clear of the cube in every view.
pub(crate) const CURVED_ARROW_RADIUS: f32 = 74.0;
/// A band along a circle around `centre` from angle `from` to `to` (radians, screen y down),
/// ending in a head at `to`; filled and outlined like the cube's facets.
pub(crate) fn curved_arrow(painter: &egui::Painter, centre: Pos2, from: f32, to: f32, hover: bool, t: Tokens, scale: f32) {
    const STEPS: u8 = 12;
    let (radius, band, head, head_length) = (CURVED_ARROW_RADIUS * scale, 4.0 * scale, 9.0 * scale, 13.0 * scale);
    let at = |angle: f32, r: f32| centre + vec2(angle.cos(), angle.sin()) * r;
    let angles: Vec<f32> = (0..=STEPS).map(|i| egui::lerp(from..=to, f32::from(i) / f32::from(STEPS))).collect();
    let fill = if hover { t.accent_soft } else { t.cube_face };
    let tip = at(to + (to - from).signum() * head_length / radius, radius);
    let head_points = [at(to, radius + head), tip, at(to, radius - head)];
    // The band is not convex: fill it as one mesh, so no seams show, and outline it once.
    let mut mesh = egui::Mesh::default();
    for &angle in &angles {
        mesh.colored_vertex(at(angle, radius - band), fill);
        mesh.colored_vertex(at(angle, radius + band), fill);
    }
    for i in 0..u32::from(STEPS) {
        mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2);
        mesh.add_triangle(2 * i + 1, 2 * i + 3, 2 * i + 2);
    }
    let head_start = mesh.vertices.len() as u32;
    for point in head_points {
        mesh.colored_vertex(point, fill);
    }
    mesh.add_triangle(head_start, head_start + 1, head_start + 2);
    painter.add(Shape::mesh(mesh));
    let mut outline: Vec<Pos2> = angles.iter().map(|a| at(*a, radius + band)).collect();
    outline.extend(head_points);
    outline.extend(angles.iter().rev().map(|a| at(*a, radius - band)));
    painter.add(Shape::closed_line(outline, Stroke::new(1.0, if hover { t.accent } else { t.cube_border })));
}
