//! A chart's scene (`statscraft_export::picture`) painted with the window's painter: the same
//! shapes the SVG and PNG files are made of.

use eframe::egui::{
    Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Stroke, epaint::PathShape, vec2,
};
use statscraft_export::picture::{Anchor, Color, Scene, Shape, on_circle};

fn color(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Paints the scene with its top left corner at `origin`; the background stays the window's
/// paper.
pub fn paint(painter: &Painter, scene: &Scene, origin: Pos2) {
    let at = |[x, y]: [f32; 2]| origin + vec2(x, y);
    for shape in &scene.shapes {
        match shape {
            Shape::Rect { x, y, w, h, fill } => {
                let rect = Rect::from_min_size(at([*x, *y]), vec2(*w, *h));
                painter.rect_filled(rect, 0.0, color(*fill));
            }
            Shape::Line {
                points,
                width,
                color: c,
            } => {
                let points = points.iter().map(|p| at(*p)).collect();
                painter.add(PathShape::line(points, Stroke::new(*width, color(*c))));
            }
            Shape::Band { upper, lower, fill } => {
                // two triangles per step: a band need not be convex, which a filled path must be
                let mut mesh = Mesh::default();
                for (i, (u, l)) in upper.iter().zip(lower).enumerate() {
                    mesh.colored_vertex(at(*u), color(*fill));
                    mesh.colored_vertex(at(*l), color(*fill));
                    if i > 0 {
                        let k = 2 * i as u32;
                        mesh.add_triangle(k - 2, k - 1, k);
                        mesh.add_triangle(k - 1, k + 1, k);
                    }
                }
                painter.add(mesh);
            }
            Shape::Polygon { points, fill } => {
                let points = points.iter().map(|p| at(*p)).collect();
                painter.add(PathShape::convex_polygon(
                    points,
                    color(*fill),
                    Stroke::NONE,
                ));
            }
            Shape::Wedge {
                center,
                radius,
                start,
                end,
                fill,
                edge,
            } => {
                let steps = ((end - start) / 0.04).ceil().max(2.0) as usize;
                let rim: Vec<Pos2> = (0..=steps)
                    .map(|k| {
                        let angle = start + (end - start) * k as f32 / steps as f32;
                        at(on_circle(*center, *radius, angle))
                    })
                    .collect();
                let mut mesh = Mesh::default();
                mesh.colored_vertex(at(*center), color(*fill));
                for (k, &p) in rim.iter().enumerate() {
                    mesh.colored_vertex(p, color(*fill));
                    if k > 0 {
                        mesh.add_triangle(0, k as u32, k as u32 + 1);
                    }
                }
                painter.add(mesh);
                // the edge between slices, which also smooths the rim the mesh leaves rough
                let whole = end - start >= std::f32::consts::TAU - 1e-4;
                let outline = match whole {
                    true => rim,
                    false => std::iter::once(at(*center)).chain(rim).collect(),
                };
                painter.add(PathShape::closed_line(
                    outline,
                    Stroke::new(1.5, color(*edge)),
                ));
            }
            Shape::Circle {
                center,
                radius,
                fill,
            } => {
                painter.circle_filled(at(*center), *radius, color(*fill));
            }
            Shape::Text {
                x,
                y,
                text,
                size,
                color: c,
                anchor,
            } => {
                let align = match anchor {
                    Anchor::Start => Align2::LEFT_CENTER,
                    Anchor::Middle => Align2::CENTER_CENTER,
                    Anchor::End => Align2::RIGHT_CENTER,
                };
                painter.text(
                    at([*x, *y]),
                    align,
                    text,
                    FontId::proportional(*size),
                    color(*c),
                );
            }
        }
    }
}
