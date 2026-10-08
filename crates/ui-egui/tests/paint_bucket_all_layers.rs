//! #1118: the Paint Bucket's "All Layers" option must reach `paint.bucket`.
//!
//! The checkbox edited `tool_options.sample_all_layers`, but the canvas click never sent it, so a
//! click on an empty layer filled the whole layer with the option on or off. This drives the real
//! app offscreen on wgpu with real egui pointer events (like `drag_preview_canvas.rs`) and skips
//! when no GPU adapter exists.

use egui::{Modifiers, PointerButton, Pos2};
use photocraft_ui_egui::PhotocraftApp;
use photocraft_ui_egui::canvas::ViewXform;
use photocraft_ui_egui::control::{ControlRequest, handle};
use serde_json::json;

type Harness = egui_kittest::Harness<'static, PhotocraftApp>;

fn harness() -> Option<Harness> {
    let built = std::panic::catch_unwind(|| {
        egui_kittest::Harness::builder().with_size(egui::vec2(900.0, 640.0)).with_max_steps(64).wgpu().build_eframe(|cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
            if let Some(rs) = cc.wgpu_render_state.as_ref() {
                app.set_wgpu(rs.clone());
            }
            app
        })
    });
    match built {
        Ok(h) => Some(h),
        Err(_) => {
            eprintln!("skipping: no GPU adapter");
            None
        }
    }
}

/// Screen point (egui points) of document point `(x, y)`.
fn screen(h: &Harness, x: f32, y: f32) -> Pos2 {
    let app = h.state();
    let v = &app.ui.views[0];
    let xf = ViewXform {
        rect: photocraft_ui_egui::rulers::content_rect(app, app.last_canvas_rect),
        zoom: v.zoom,
        center: v.center,
        flip: app.ui.view.flip_horizontal,
    };
    xf.to_screen(x, y)
}

fn click(h: &mut Harness, x: f32, y: f32) {
    let p = screen(h, x, y);
    h.event(egui::Event::PointerMoved(p));
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.step();
    }
    h.run_steps(3);
}

/// Pixels the Paint Bucket fills on an empty layer above a Background whose left half is black,
/// clicking the white right half.
fn filled(all_layers: bool) -> Option<usize> {
    let mut h = harness()?;
    {
        let app = h.state_mut();
        app.run("file.new", json!({"width": 400, "height": 300, "background": "white"})).expect("new");
        app.sync_views();
        app.ui.extras.rulers = false;
        app.run("select.rect", json!({"x": 0, "y": 0, "width": 200, "height": 300})).expect("select");
        app.run("tools.setColors", json!({"foreground": "#000000"})).expect("colors");
        app.run("edit.fill", json!({"contents": "foreground"})).expect("fill");
        app.run("select.deselect", json!({})).expect("deselect");
        app.run("layer.new.layer", json!({})).expect("layer");
        app.run("tools.setColors", json!({"foreground": "#ff0000"})).expect("colors");
        app.ui.tool_options.sample_all_layers = all_layers;
        app.ui.tool_options.anti_alias = false;
    }
    let ctx = h.ctx.clone();
    let (req, _rx) = ControlRequest::new("ui.set", json!({"tool": "paintBucket", "zoom": 1.0, "center": [200, 150]}));
    handle(h.state_mut(), &ctx, &req);
    h.run_steps(3);
    click(&mut h, 300.0, 150.0);
    let st = h.state().session.active()?;
    let surf = st.active_layer.and_then(|id| st.doc.layer(id))?.surface()?;
    Some((0..300).flat_map(|y| (0..400).map(move |x| (x, y))).filter(|&(x, y)| surf.rgba(x, y)[3] > 0.5).count())
}

#[test]
fn paint_bucket_all_layers_fills_only_the_region_seen_in_the_merged_image() {
    let (Some(off), Some(on)) = (filled(false), filled(true)) else { return };
    assert_eq!(off, 400 * 300, "without All Layers the empty layer is one region");
    assert_eq!(on, 200 * 300, "with All Layers the black half stops the fill");
}
