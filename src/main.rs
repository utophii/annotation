use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, DrawingArea, Entry,
    EventControllerFocus, EventControllerKey, GestureDrag, Label, Orientation, Overlay,
};

use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const APP_ID: &str = "dev.local.annotation";
const SV_SIZE: i32 = 150;
const HUE_WIDTH: i32 = 20;

#[derive(Clone)]
struct Stroke {
    color: (f64, f64, f64),
    width: f64,
    points: Vec<(f64, f64)>,
}

struct DrawState {
    strokes: Vec<Stroke>,
    current: Option<Stroke>,
    color: (f64, f64, f64),
    hue: f64,
    sat: f64,
    val: f64,
    width: f64,
}

impl DrawState {
    fn new() -> Self {
        let hue = 0.0;
        let sat = 0.85;
        let val = 1.0;
        Self {
            strokes: Vec::new(),
            current: None,
            color: hsv_to_rgb(hue, sat, val),
            hue,
            sat,
            val,
            width: 4.0,
        }
    }

    fn clamp_width(w: f64) -> f64 {
        w.clamp(1.0, 60.0)
    }

    fn set_hsv(&mut self, h: f64, s: f64, v: f64) {
        self.hue = h.rem_euclid(360.0);
        self.sat = s.clamp(0.0, 1.0);
        self.val = v.clamp(0.0, 1.0);
        self.color = hsv_to_rgb(self.hue, self.sat, self.val);
    }

    fn set_rgb(&mut self, r: f64, g: f64, b: f64) {
        self.color = (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
        let (h, s, v) = rgb_to_hsv(self.color.0, self.color.1, self.color.2);
        self.hue = h;
        self.sat = s;
        self.val = v;
    }
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r1 + m, g1 + m, b1 + m)
}

fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta.abs() < 1e-9 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let s = if max.abs() < 1e-9 { 0.0 } else { delta / max };
    (h, s, max)
}

fn parse_hex_color(input: &str) -> Option<(u8, u8, u8)> {
    let s = input.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

fn to_hex_string(color: (f64, f64, f64)) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.0 * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.1 * 255.0).round().clamp(0.0, 255.0) as u8,
        (color.2 * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    load_css();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("annotation")
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace("annotation");

    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);

    window.set_exclusive_zone(0);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.add_css_class("annotation-window");

    let state = Rc::new(RefCell::new(DrawState::new()));

    // --- Холст для рисования ---
    let drawing_area = DrawingArea::new();
    drawing_area.set_hexpand(true);
    drawing_area.set_vexpand(true);
    drawing_area.set_can_focus(true);

    {
        let state = state.clone();
        drawing_area.set_draw_func(move |_area, cr, _w, _h| {
            draw(cr, &state.borrow());
        });
    }

    let gesture = GestureDrag::new();
    gesture.set_button(0);

    {
        let state = state.clone();
        let area = drawing_area.clone();
        gesture.connect_drag_begin(move |_gesture, x, y| {
            let mut st = state.borrow_mut();
            let stroke = Stroke {
                color: st.color,
                width: st.width,
                points: vec![(x, y)],
            };
            st.current = Some(stroke);
            drop(st);
            area.queue_draw();
        });
    }
    {
        let state = state.clone();
        let area = drawing_area.clone();
        gesture.connect_drag_update(move |gesture, dx, dy| {
            if let Some((sx, sy)) = gesture.start_point() {
                let mut st = state.borrow_mut();
                if let Some(stroke) = st.current.as_mut() {
                    stroke.points.push((sx + dx, sy + dy));
                }
                drop(st);
                area.queue_draw();
            }
        });
    }
    {
        let state = state.clone();
        let area = drawing_area.clone();
        gesture.connect_drag_end(move |_gesture, _dx, _dy| {
            let mut st = state.borrow_mut();
            if let Some(stroke) = st.current.take() {
                if stroke.points.len() > 1 {
                    st.strokes.push(stroke);
                }
            }
            drop(st);
            area.queue_draw();
        });
    }
    drawing_area.add_controller(gesture);

    let toolbar = build_toolbar(&state, &drawing_area, app);

    let overlay = Overlay::new();
    overlay.set_child(Some(&drawing_area));
    overlay.add_overlay(&toolbar);

    window.set_child(Some(&overlay));

    let key_controller = EventControllerKey::new();
    {
        let state = state.clone();
        let area = drawing_area.clone();
        let app = app.clone();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, _modifiers| {
            match keyval {
                gdk::Key::Escape | gdk::Key::q | gdk::Key::Q => {
                    app.quit();
                    glib::Propagation::Stop
                }
                gdk::Key::c | gdk::Key::C => {
                    let mut st = state.borrow_mut();
                    st.strokes.clear();
                    st.current = None;
                    drop(st);
                    area.queue_draw();
                    glib::Propagation::Stop
                }
                gdk::Key::plus | gdk::Key::KP_Add | gdk::Key::equal => {
                    let mut st = state.borrow_mut();
                    st.width = DrawState::clamp_width(st.width + 1.0);
                    glib::Propagation::Stop
                }
                gdk::Key::minus | gdk::Key::KP_Subtract => {
                    let mut st = state.borrow_mut();
                    st.width = DrawState::clamp_width(st.width - 1.0);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
    }
    window.add_controller(key_controller);

    window.set_decorated(false);
    window.present();
    drawing_area.grab_focus();
}

fn draw(cr: &cairo::Context, state: &DrawState) {
    cr.set_operator(cairo::Operator::Clear);
    let _ = cr.paint();
    cr.set_operator(cairo::Operator::Over);

    for stroke in state.strokes.iter().chain(state.current.iter()) {
        if stroke.points.len() < 2 {
            continue;
        }
        cr.set_source_rgba(stroke.color.0, stroke.color.1, stroke.color.2, 1.0);
        cr.set_line_width(stroke.width);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);

        let mut points = stroke.points.iter();
        if let Some((x0, y0)) = points.next() {
            cr.move_to(*x0, *y0);
            for (x, y) in points {
                cr.line_to(*x, *y);
            }
        }
        let _ = cr.stroke();
    }
}

fn refresh_picker_ui(
    state: &Rc<RefCell<DrawState>>,
    sv_area: &DrawingArea,
    hue_area: &DrawingArea,
    preview: &DrawingArea,
    hex_entry: &Entry,
) {
    sv_area.queue_draw();
    hue_area.queue_draw();
    preview.queue_draw();
    if !hex_entry.has_focus() {
        let color = state.borrow().color;
        hex_entry.set_text(&to_hex_string(color));
    }
}

fn pick_from_sv(
    state: &Rc<RefCell<DrawState>>,
    sv_area: &DrawingArea,
    hue_area: &DrawingArea,
    preview: &DrawingArea,
    hex_entry: &Entry,
    x: f64,
    y: f64,
) {
    let w = sv_area.width().max(1) as f64;
    let h = sv_area.height().max(1) as f64;
    let s = (x / w).clamp(0.0, 1.0);
    let v = (1.0 - y / h).clamp(0.0, 1.0);
    let hue = state.borrow().hue;
    state.borrow_mut().set_hsv(hue, s, v);
    refresh_picker_ui(state, sv_area, hue_area, preview, hex_entry);
}

fn pick_from_hue(
    state: &Rc<RefCell<DrawState>>,
    sv_area: &DrawingArea,
    hue_area: &DrawingArea,
    preview: &DrawingArea,
    hex_entry: &Entry,
    y: f64,
) {
    let h = hue_area.height().max(1) as f64;
    let hue = (y / h).clamp(0.0, 1.0) * 360.0;
    let (s, v) = {
        let st = state.borrow();
        (st.sat, st.val)
    };
    state.borrow_mut().set_hsv(hue, s, v);
    refresh_picker_ui(state, sv_area, hue_area, preview, hex_entry);
}

fn commit_hex(
    entry: &Entry,
    state: &Rc<RefCell<DrawState>>,
    sv_area: &DrawingArea,
    hue_area: &DrawingArea,
    preview: &DrawingArea,
) {
    let text = entry.text();
    if let Some((r, g, b)) = parse_hex_color(&text) {
        state
            .borrow_mut()
            .set_rgb(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        sv_area.queue_draw();
        hue_area.queue_draw();
        preview.queue_draw();
    } else {
        let color = state.borrow().color;
        entry.set_text(&to_hex_string(color));
    }
}

fn build_color_picker(state: &Rc<RefCell<DrawState>>) -> GtkBox {
    let picker_row = GtkBox::new(Orientation::Horizontal, 8);

    let sv_area = DrawingArea::new();
    sv_area.set_content_width(SV_SIZE);
    sv_area.set_content_height(SV_SIZE);
    sv_area.add_css_class("sv-square");
    sv_area.set_cursor_from_name(Some("crosshair"));

    let hue_area = DrawingArea::new();
    hue_area.set_content_width(HUE_WIDTH);
    hue_area.set_content_height(SV_SIZE);
    hue_area.add_css_class("hue-strip");
    hue_area.set_cursor_from_name(Some("crosshair"));

    let preview = DrawingArea::new();
    preview.set_content_width(HUE_WIDTH);
    preview.set_content_height(26);
    preview.add_css_class("color-preview");

    let hex_entry = Entry::new();
    hex_entry.set_placeholder_text(Some("#RRGGBB"));
    hex_entry.set_max_width_chars(8);
    hex_entry.add_css_class("hex-entry");
    hex_entry.set_text(&to_hex_string(state.borrow().color));

    {
        let state = state.clone();
        sv_area.set_draw_func(move |_area, cr, w, h| {
            let hue = state.borrow().hue;
            let (hr, hg, hb) = hsv_to_rgb(hue, 1.0, 1.0);
            let w = w as f64;
            let h = h as f64;

            cr.set_source_rgb(hr, hg, hb);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();

            let sat_grad = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
            sat_grad.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 1.0);
            sat_grad.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
            let _ = cr.set_source(&sat_grad);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();

            let val_grad = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
            val_grad.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
            val_grad.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
            let _ = cr.set_source(&val_grad);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();

            let (sat, val) = {
                let st = state.borrow();
                (st.sat, st.val)
            };
            let mx = sat * w;
            let my = (1.0 - val) * h;
            cr.set_line_width(2.0);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.arc(mx, my, 6.0, 0.0, PI * 2.0);
            let _ = cr.stroke();
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.arc(mx, my, 7.5, 0.0, PI * 2.0);
            let _ = cr.stroke();
        });
    }

    {
        let state = state.clone();
        hue_area.set_draw_func(move |_area, cr, w, h| {
            let w = w as f64;
            let h = h as f64;

            let grad = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
            let steps = 12;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let (r, g, b) = hsv_to_rgb(t * 360.0, 1.0, 1.0);
                grad.add_color_stop_rgb(t, r, g, b);
            }
            let _ = cr.set_source(&grad);
            cr.rectangle(0.0, 0.0, w, h);
            let _ = cr.fill();

            let hue = state.borrow().hue;
            let my = (hue / 360.0) * h;
            cr.set_line_width(2.0);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.rectangle(0.0, my - 2.0, w, 4.0);
            let _ = cr.stroke();
        });
    }

    {
        let state = state.clone();
        preview.set_draw_func(move |_area, cr, w, h| {
            let st = state.borrow();
            cr.set_source_rgb(st.color.0, st.color.1, st.color.2);
            cr.rectangle(0.0, 0.0, w as f64, h as f64);
            let _ = cr.fill();
        });
    }

    let sv_drag = GestureDrag::new();
    {
        let state = state.clone();
        let sv_area_c = sv_area.clone();
        let hue_area_c = hue_area.clone();
        let preview_c = preview.clone();
        let hex_entry_c = hex_entry.clone();
        sv_drag.connect_drag_begin(move |_g, x, y| {
            pick_from_sv(&state, &sv_area_c, &hue_area_c, &preview_c, &hex_entry_c, x, y);
        });
    }
    {
        let state = state.clone();
        let sv_area_c = sv_area.clone();
        let hue_area_c = hue_area.clone();
        let preview_c = preview.clone();
        let hex_entry_c = hex_entry.clone();
        sv_drag.connect_drag_update(move |g, dx, dy| {
            if let Some((sx, sy)) = g.start_point() {
                pick_from_sv(
                    &state,
                    &sv_area_c,
                    &hue_area_c,
                    &preview_c,
                    &hex_entry_c,
                    sx + dx,
                    sy + dy,
                );
            }
        });
    }
    sv_area.add_controller(sv_drag);

    let hue_drag = GestureDrag::new();
    {
        let state = state.clone();
        let sv_area_c = sv_area.clone();
        let hue_area_c = hue_area.clone();
        let preview_c = preview.clone();
        let hex_entry_c = hex_entry.clone();
        hue_drag.connect_drag_begin(move |_g, _x, y| {
            pick_from_hue(&state, &sv_area_c, &hue_area_c, &preview_c, &hex_entry_c, y);
        });
    }
    {
        let state = state.clone();
        let sv_area_c = sv_area.clone();
        let hue_area_c = hue_area.clone();
        let preview_c = preview.clone();
        let hex_entry_c = hex_entry.clone();
        hue_drag.connect_drag_update(move |g, _dx, dy| {
            if let Some((_sx, sy)) = g.start_point() {
                pick_from_hue(
                    &state,
                    &sv_area_c,
                    &hue_area_c,
                    &preview_c,
                    &hex_entry_c,
                    sy + dy,
                );
            }
        });
    }
    hue_area.add_controller(hue_drag);

    {
        let state = state.clone();
        let sv_area_c = sv_area.clone();
        let hue_area_c = hue_area.clone();
        let preview_c = preview.clone();
        hex_entry.connect_activate(move |entry| {
            commit_hex(entry, &state, &sv_area_c, &hue_area_c, &preview_c);
        });
    }
    {
        let state = state.clone();
        let sv_area_c = sv_area.clone();
        let hue_area_c = hue_area.clone();
        let preview_c = preview.clone();
        let entry_c = hex_entry.clone();
        let focus_ctrl = EventControllerFocus::new();
        focus_ctrl.connect_leave(move |_| {
            commit_hex(&entry_c, &state, &sv_area_c, &hue_area_c, &preview_c);
        });
        hex_entry.add_controller(focus_ctrl);
    }

    let side_col = GtkBox::new(Orientation::Vertical, 6);
    side_col.append(&preview);
    side_col.append(&hex_entry);

    picker_row.append(&sv_area);
    picker_row.append(&hue_area);
    picker_row.append(&side_col);

    picker_row
}

fn build_presets_row(
    state: &Rc<RefCell<DrawState>>,
    sv_area: &DrawingArea,
    hue_area: &DrawingArea,
    preview: &DrawingArea,
    hex_entry: &Entry,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 4);
    let colors: [(f64, f64, f64); 8] = [
        (1.0, 0.15, 0.15),
        (1.0, 0.55, 0.0),
        (1.0, 0.9, 0.1),
        (0.15, 0.8, 0.2),
        (0.1, 0.75, 0.7),
        (0.15, 0.5, 1.0),
        (0.6, 0.2, 1.0),
        (1.0, 1.0, 1.0),
    ];

    for (r, g, b) in colors {
        let btn = Button::new();
        btn.add_css_class("swatch-btn");
        let swatch = GtkBox::new(Orientation::Horizontal, 0);
        swatch.set_size_request(18, 18);

        let css = format!(
            "box {{ background-color: rgb({}, {}, {}); border-radius: 4px; }}",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8
        );
        let provider = CssProvider::new();
        provider.load_from_data(&css);
        swatch
            .style_context()
            .add_provider(&provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
        btn.set_child(Some(&swatch));

        {
            let state = state.clone();
            let sv_area = sv_area.clone();
            let hue_area = hue_area.clone();
            let preview = preview.clone();
            let hex_entry = hex_entry.clone();
            btn.connect_clicked(move |_| {
                state.borrow_mut().set_rgb(r, g, b);
                refresh_picker_ui(&state, &sv_area, &hue_area, &preview, &hex_entry);
            });
        }
        row.append(&btn);
    }

    row
}

fn build_toolbar(
    state: &Rc<RefCell<DrawState>>,
    drawing_area: &DrawingArea,
    app: &Application,
) -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 8);
    outer.set_halign(gtk4::Align::End);
    outer.set_valign(gtk4::Align::Start);
    outer.set_margin_top(16);
    outer.set_margin_end(16);
    outer.add_css_class("toolbar-panel");

    let row1 = GtkBox::new(Orientation::Horizontal, 8);

    let title = Label::new(Some("annotation"));
    title.add_css_class("hint-label");
    row1.append(&title);

    let minus_btn = Button::with_label("−");
    let plus_btn = Button::with_label("+");
    minus_btn.set_tooltip_text(Some("Тоньше (-)"));
    plus_btn.set_tooltip_text(Some("Толще (+)"));
    {
        let state = state.clone();
        minus_btn.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            st.width = DrawState::clamp_width(st.width - 1.0);
        });
    }
    {
        let state = state.clone();
        plus_btn.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            st.width = DrawState::clamp_width(st.width + 1.0);
        });
    }
    row1.append(&minus_btn);
    row1.append(&plus_btn);

    let clear_btn = Button::with_label("Очистить (C)");
    {
        let state = state.clone();
        let area = drawing_area.clone();
        clear_btn.connect_clicked(move |_| {
            let mut st = state.borrow_mut();
            st.strokes.clear();
            st.current = None;
            drop(st);
            area.queue_draw();
        });
    }
    row1.append(&clear_btn);

    let quit_btn = Button::with_label("Выход (Esc)");
    quit_btn.add_css_class("quit-btn");
    {
        let app = app.clone();
        quit_btn.connect_clicked(move |_| {
            app.quit();
        });
    }
    row1.append(&quit_btn);

    let picker = build_color_picker(state);

    let sv_area = picker
        .first_child()
        .and_downcast::<DrawingArea>()
        .expect("sv_area");
    let hue_area = sv_area
        .next_sibling()
        .and_downcast::<DrawingArea>()
        .expect("hue_area");
    let side_col = hue_area
        .next_sibling()
        .and_downcast::<GtkBox>()
        .expect("side_col");
    let preview = side_col
        .first_child()
        .and_downcast::<DrawingArea>()
        .expect("preview");
    let hex_entry = preview
        .next_sibling()
        .and_downcast::<Entry>()
        .expect("hex_entry");

    let presets = build_presets_row(state, &sv_area, &hue_area, &preview, &hex_entry);

    outer.append(&row1);
    outer.append(&picker);
    outer.append(&presets);

    outer
}

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_data(
        r#"
        window.annotation-window {
            background-color: transparent;
        }

        .toolbar-panel {
            background-color: rgba(20, 20, 24, 0.82);
            border-radius: 12px;
            padding: 10px 12px;
        }

        .toolbar-panel label,
        .toolbar-panel button,
        .toolbar-panel entry {
            color: #f2f2f2;
        }

        .hint-label {
            font-family: monospace;
            font-size: 12px;
            opacity: 0.85;
            margin-right: 4px;
        }

        .quit-btn {
            background-color: rgba(200, 40, 40, 0.6);
        }

        .sv-square,
        .hue-strip {
            border-radius: 4px;
        }

        .color-preview {
            border-radius: 4px;
            border: 1px solid rgba(255, 255, 255, 0.4);
        }

        .hex-entry {
            min-width: 90px;
            font-family: monospace;
        }

        .swatch-btn {
            padding: 2px;
            min-width: 22px;
            min-height: 22px;
        }
        "#,
    );

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}