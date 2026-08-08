use std::cell::RefCell;
use std::rc::Rc;

use gtk4::cairo;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, ColorButton, CssProvider,
    DrawingArea, EventControllerKey, GestureDrag, Label, Orientation, Overlay,
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
    let toolbar = GtkBox::new(Orientation::Horizontal, 8);
    toolbar.set_halign(gtk4::Align::End);
    toolbar.set_valign(gtk4::Align::Start);
    toolbar.set_margin_top(16);
    toolbar.set_margin_end(16);
    toolbar.add_css_class("toolbar-panel");

    let title = Label::new(Some("annotation"));
    title.add_css_class("hint-label");
    toolbar.append(&title);

    let palette = GtkBox::new(Orientation::Horizontal, 4);
    let colors: [(f64, f64, f64, &str); 6] = [
        (1.0, 0.15, 0.15, "swatch-red"),
        (1.0, 0.7, 0.0, "swatch-orange"),
        (1.0, 0.95, 0.1, "swatch-yellow"),
        (0.1, 0.85, 0.2, "swatch-green"),
        (0.1, 0.55, 1.0, "swatch-blue"),
        (1.0, 1.0, 1.0, "swatch-white"),
    ];

    for (r, g, b, css_class) in colors {
        let btn = Button::new();
        btn.add_css_class("swatch-btn");
        let swatch = GtkBox::new(Orientation::Horizontal, 0);
        swatch.set_size_request(18, 18);
        swatch.add_css_class(css_class);
        btn.set_child(Some(&swatch));
        {
            let state = state.clone();
            btn.connect_clicked(move |_| {
                let mut st = state.borrow_mut();
                st.color = (r, g, b);
            });
        }
        palette.append(&btn);
    }
    toolbar.append(&palette);

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
    toolbar.append(&minus_btn);
    toolbar.append(&plus_btn);

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
    toolbar.append(&clear_btn);

    let quit_btn = Button::with_label("Выход (Esc)");
    quit_btn.add_css_class("quit-btn");
    {
        let app = app.clone();
        quit_btn.connect_clicked(move |_| {
            app.quit();
        });
    }
    toolbar.append(&quit_btn);

    toolbar
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

        .swatch-red    { background-color: #ff2626; border-radius: 4px; }
        .swatch-orange { background-color: #ffb300; border-radius: 4px; }
        .swatch-yellow { background-color: #fff21a; border-radius: 4px; }
        .swatch-green  { background-color: #1ad937; border-radius: 4px; }
        .swatch-blue   { background-color: #1a8cff; border-radius: 4px; }
        .swatch-white  { background-color: #ffffff; border-radius: 4px; }
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
