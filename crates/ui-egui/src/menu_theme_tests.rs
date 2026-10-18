//! Exercise the painted main menu while switching themes in a live UI context.
use crate::{FigApp, NoServices};
use egui::{Color32, Context, RawInput, Shape, pos2, vec2};
use figcraft_engine::Session;

fn has_fill(shapes: &[Shape], color: Color32) -> bool {
    shapes.iter().any(|s| match s {
        Shape::Vec(v) => has_fill(v, color),
        Shape::Rect(r) => r.fill == color && r.rect.width() >= 240.0 && r.rect.height() > 100.0,
        _ => false,
    })
}

fn text_color(shapes: &[Shape], label: &str) -> Option<Color32> {
    shapes.iter().find_map(|s| match s {
        Shape::Vec(v) => text_color(v, label),
        Shape::Text(t) if t.galley.text() == label => {
            let color = t.galley.rows.first()?.visuals.mesh.vertices.first()?.color;
            Some(t.override_text_color.unwrap_or(if color == Color32::PLACEHOLDER { t.fallback_color } else { color }))
        }
        _ => None,
    })
}

#[test]
fn open_main_menu_repaints_its_surface_and_labels_when_theme_changes() {
    let ctx = Context::default();
    let mut app = FigApp::new(Session::new(), Box::new(NoServices));
    app.ui.main_menu = true;
    for (theme, bg, fg) in [
        ("light", Color32::WHITE, Color32::from_gray(30)),
        ("dark", Color32::from_gray(30), Color32::WHITE),
        ("light", Color32::WHITE, Color32::from_gray(30)),
    ] {
        app.session.prefs.theme = theme.into();
        let mut shapes = Vec::new();
        for _ in 0..3 {
            let input = RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0))), ..Default::default() };
            let output = ctx.run_ui(input, |ui| app.ui(ui));
            shapes = output.shapes.into_iter().map(|s| s.shape).collect();
        }
        assert!(has_fill(&shapes, bg), "menu surface follows {theme}");
        assert_eq!(text_color(&shapes, "File"), Some(fg), "menu label follows {theme}");
    }
}
