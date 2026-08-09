use std::cell::RefCell;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider,
    DrawingArea, Entry, EventControllerKey, GestureDrag, Label, Orientation, Overlay, Scale,
};

use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const APP_ID: &str = "dev.local.annotation";

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
    width: f64,
}

impl DrawState {
    fn new() -> Self {
        Self {
            strokes: Vec::new(),
            current: None,
            color: (1.0, 0.15, 0.15),
            width: 4.0,
        }
    }

    fn clamp_width(w: f64) -> f64 {
        w.clamp(1.0, 60.0)
    }
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

fn build_toolbar(
    state: &Rc<RefCell<DrawState>>,
    drawing_area: &DrawingArea,
    app: &Application,
) -> GtkBox {
    let outer = GtkBox::new(Orientation::Vertical, 6);
    outer.set_halign(gtk4::Align::End);
    outer.set_valign(gtk4::Align::Start);
    outer.set_margin_top(16);
    outer.set_margin_end(16);
    outer.add_css_class("toolbar-panel");

    let row1 = GtkBox::new(Orientation::Horizontal, 8);

    let title = Label::new(Some("annotation"));
    title.add_css_class("hint-label");
    row1.append(&title);

    let preview = DrawingArea::new();
    preview.set_content_width(26);
    preview.set_content_height(26);
    preview.add_css_class("color-preview");
    {
        let state = state.clone();
        preview.set_draw_func(move |_area, cr, w, h| {
            let st = state.borrow();
            cr.set_source_rgb(st.color.0, st.color.1, st.color.2);
            cr.rectangle(0.0, 0.0, w as f64, h as f64);
            let _ = cr.fill();
        });
    }

    let r_scale = Scale::with_range(Orientation::Horizontal, 0.0, 255.0, 1.0);
    let g_scale = Scale::with_range(Orientation::Horizontal, 0.0, 255.0, 1.0);
    let b_scale = Scale::with_range(Orientation::Horizontal, 0.0, 255.0, 1.0);
    for s in [&r_scale, &g_scale, &b_scale] {
        s.set_draw_value(false);
        s.set_size_request(90, -1);
    }
    {
        let st = state.borrow();
        r_scale.set_value(st.color.0 * 255.0);
        g_scale.set_value(st.color.1 * 255.0);
        b_scale.set_value(st.color.2 * 255.0);
    }

    let hex_entry = Entry::new();
    hex_entry.set_placeholder_text(Some("#RRGGBB"));
    hex_entry.set_max_width_chars(8);
    hex_entry.add_css_class("hex-entry");

    let update_from_sliders: Rc<dyn Fn()> = {
        let state = state.clone();
        let preview = preview.clone();
        let r_scale = r_scale.clone();
        let g_scale = g_scale.clone();
        let b_scale = b_scale.clone();
        Rc::new(move || {
            let r = r_scale.value() / 255.0;
            let g = g_scale.value() / 255.0;
            let b = b_scale.value() / 255.0;
            state.borrow_mut().color = (r, g, b);
            preview.queue_draw();
        })
    };
    {
        let cb = update_from_sliders.clone();
        r_scale.connect_value_changed(move |_| cb());
    }
    {
        let cb = update_from_sliders.clone();
        g_scale.connect_value_changed(move |_| cb());
    }
    {
        let cb = update_from_sliders.clone();
        b_scale.connect_value_changed(move |_| cb());
    }

    {
        let state = state.clone();
        let preview = preview.clone();
        let r_scale = r_scale.clone();
        let g_scale = g_scale.clone();
        let b_scale = b_scale.clone();
        hex_entry.connect_activate(move |entry| {
            let text = entry.text();
            if let Some((r, g, b)) = parse_hex_color(&text) {
                state.borrow_mut().color =
                    (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
                r_scale.set_value(r as f64);
                g_scale.set_value(g as f64);
                b_scale.set_value(b as f64);
                preview.queue_draw();
            }
        });
    }

    let colors: [(f64, f64, f64); 6] = [
        (1.0, 0.15, 0.15),
        (1.0, 0.7, 0.0),
        (1.0, 0.95, 0.1),
        (0.1, 0.85, 0.2),
        (0.1, 0.55, 1.0),
        (1.0, 1.0, 1.0),
    ];
    let palette = GtkBox::new(Orientation::Horizontal, 4);
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
            let preview = preview.clone();
            let r_scale = r_scale.clone();
            let g_scale = g_scale.clone();
            let b_scale = b_scale.clone();
            btn.connect_clicked(move |_| {
                state.borrow_mut().color = (r, g, b);
                r_scale.set_value(r * 255.0);
                g_scale.set_value(g * 255.0);
                b_scale.set_value(b * 255.0);
                preview.queue_draw();
            });
        }
        palette.append(&btn);
    }
    row1.append(&palette);

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

    let row2 = GtkBox::new(Orientation::Horizontal, 6);
    row2.append(&Label::new(Some("R")));
    row2.append(&r_scale);
    row2.append(&Label::new(Some("G")));
    row2.append(&g_scale);
    row2.append(&Label::new(Some("B")));
    row2.append(&b_scale);
    row2.append(&preview);
    row2.append(&hex_entry);

    outer.append(&row1);
    outer.append(&row2);

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
            background-color: rgba(20, 20, 24, 0.78);
            border-radius: 12px;
            padding: 8px 10px;
        }

        .toolbar-panel label,
        .toolbar-panel button {
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

        .swatch-btn {
            padding: 2px;
            min-width: 22px;
            min-height: 22px;
        }

        .color-preview {
            border-radius: 4px;
            border: 1px solid rgba(255,255,255,0.4);
        }

        .hex-entry {
            min-width: 90px;
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
