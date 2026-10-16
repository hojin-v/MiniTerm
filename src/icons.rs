use egui::{
    Color32, Pos2, Response, Sense, Shape, Stroke, Ui, Vec2, Widget, WidgetInfo, WidgetType,
};
use std::f32::consts::PI;

#[derive(Clone, Copy)]
pub enum Icon {
    Add,
    Pin,
    Bell,
    Settings,
    Hide,
    Expand,
    Collapse,
    Close,
    Save,
    Link,
    Unlink,
    Power,
    Return,
    Check,
    Terminal,
    Prompt,
    Layout,
    Summary,
    Sun,
    Moon,
}

pub struct IconButton<'a> {
    icon: Icon,
    tooltip: &'a str,
    selected: bool,
    badge: bool,
    danger: bool,
    size: f32,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: Icon, tooltip: &'a str) -> Self {
        Self {
            icon,
            tooltip,
            selected: false,
            badge: false,
            danger: false,
            size: 22.0,
        }
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn badge(mut self, badge: bool) -> Self {
        self.badge = badge;
        self
    }
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::click());
        response.widget_info(|| {
            WidgetInfo::selected(
                WidgetType::Button,
                ui.is_enabled(),
                self.selected,
                self.tooltip,
            )
        });
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let accent = crate::palette::rgb(6, !ui.visuals().dark_mode);
            let accent = Color32::from_rgb(accent.r, accent.g, accent.b);
            if self.selected {
                ui.painter()
                    .rect_filled(rect, 5.0, accent.gamma_multiply(0.16));
            } else if response.hovered()
                || response.has_focus()
                || response.is_pointer_button_down_on()
            {
                ui.painter().rect_filled(rect, 5.0, visuals.bg_fill);
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect.shrink(0.5),
                    5.0,
                    Stroke::new(1.0, accent),
                    egui::StrokeKind::Inside,
                );
            }
            let color = if !ui.is_enabled() {
                ui.visuals().weak_text_color().gamma_multiply(0.45)
            } else if self.danger {
                ui.visuals().error_fg_color
            } else if self.selected {
                accent
            } else {
                visuals.fg_stroke.color
            };
            let icon_size = self.size * 0.76;
            paint(
                ui.painter(),
                self.icon,
                Pos2::new(rect.left() + self.size * 0.5, rect.center().y),
                icon_size,
                color,
                self.selected,
            );
            if self.badge {
                let center = rect.right_top() + Vec2::new(-3.5, 3.5);
                ui.painter()
                    .circle_filled(center, 2.7, Color32::from_rgb(214, 66, 83));
                ui.painter()
                    .circle_stroke(center, 2.7, Stroke::new(1.0, ui.visuals().panel_fill));
            }
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(self.tooltip)
            .on_disabled_hover_text(self.tooltip)
    }
}

fn paint(
    painter: &egui::Painter,
    icon: Icon,
    center: Pos2,
    size: f32,
    color: Color32,
    selected: bool,
) {
    let point = |x: f32, y: f32| center + Vec2::new(x - 12.0, y - 12.0) * (size / 24.0);
    let stroke = Stroke::new((size / 24.0 * 1.9).max(1.2), color);
    let line = |coordinates: &[[f32; 2]]| {
        painter.add(Shape::line(
            coordinates.iter().map(|[x, y]| point(*x, *y)).collect(),
            stroke,
        ));
    };
    match icon {
        Icon::Add => {
            line(&[[4.0, 12.0], [20.0, 12.0]]);
            line(&[[12.0, 4.0], [12.0, 20.0]]);
        }
        Icon::Close => {
            line(&[[5.0, 5.0], [19.0, 19.0]]);
            line(&[[19.0, 5.0], [5.0, 19.0]]);
        }
        Icon::Check => line(&[[4.0, 12.0], [9.0, 17.0], [20.0, 6.0]]),
        Icon::Pin => {
            let angle: f32 = if selected { 0.0 } else { 0.6 };
            let rotated = |x: f32, y: f32| {
                let x = x - 12.0;
                let y = y - 12.0;
                point(
                    12.0 + x * angle.cos() - y * angle.sin(),
                    12.0 + x * angle.sin() + y * angle.cos(),
                )
            };
            painter.add(Shape::closed_line(
                [
                    [8.0, 3.0],
                    [16.0, 3.0],
                    [15.0, 10.0],
                    [18.0, 14.0],
                    [6.0, 14.0],
                    [9.0, 10.0],
                ]
                .into_iter()
                .map(|[x, y]| rotated(x, y))
                .collect(),
                stroke,
            ));
            painter.line_segment([rotated(12.0, 14.0), rotated(12.0, 22.0)], stroke);
        }
        Icon::Bell => {
            line(&[
                [5.0, 18.0],
                [7.0, 15.0],
                [7.0, 10.0],
                [8.0, 7.0],
                [10.0, 5.0],
                [14.0, 5.0],
                [16.0, 7.0],
                [17.0, 10.0],
                [17.0, 15.0],
                [19.0, 18.0],
                [5.0, 18.0],
            ]);
            line(&[[12.0, 2.0], [12.0, 5.0]]);
            line(&[[9.5, 21.0], [12.0, 22.0], [14.5, 21.0]]);
        }
        Icon::Settings => {
            let teeth = (0..32)
                .map(|i| {
                    let radius = if i % 4 < 2 { 10.0 } else { 8.0 };
                    let angle = i as f32 * PI / 16.0;
                    point(12.0 + radius * angle.cos(), 12.0 + radius * angle.sin())
                })
                .collect();
            painter.add(Shape::closed_line(teeth, stroke));
            painter.circle_stroke(center, size * 0.15, stroke);
        }
        Icon::Hide => {
            line(&[
                [3.0, 12.0],
                [6.0, 8.0],
                [10.0, 6.0],
                [14.0, 6.0],
                [18.0, 8.0],
                [21.0, 12.0],
                [18.0, 16.0],
                [14.0, 18.0],
                [10.0, 18.0],
                [6.0, 16.0],
                [3.0, 12.0],
            ]);
            painter.circle_stroke(center, size * 0.14, stroke);
            line(&[[3.0, 3.0], [21.0, 21.0]]);
        }
        Icon::Expand => {
            line(&[[3.0, 9.0], [3.0, 3.0], [9.0, 3.0]]);
            line(&[[15.0, 21.0], [21.0, 21.0], [21.0, 15.0]]);
            line(&[[3.0, 3.0], [10.0, 10.0]]);
            line(&[[21.0, 21.0], [14.0, 14.0]]);
        }
        Icon::Collapse => {
            line(&[[3.0, 9.0], [9.0, 9.0], [9.0, 3.0]]);
            line(&[[21.0, 15.0], [15.0, 15.0], [15.0, 21.0]]);
            line(&[[3.0, 3.0], [9.0, 9.0]]);
            line(&[[21.0, 21.0], [15.0, 15.0]]);
        }
        Icon::Save => {
            line(&[
                [3.0, 3.0],
                [17.0, 3.0],
                [21.0, 7.0],
                [21.0, 21.0],
                [3.0, 21.0],
                [3.0, 3.0],
            ]);
            line(&[[7.0, 3.0], [7.0, 9.0], [16.0, 9.0], [16.0, 3.0]]);
            line(&[[7.0, 21.0], [7.0, 14.0], [17.0, 14.0], [17.0, 21.0]]);
        }
        Icon::Link | Icon::Unlink => {
            line(&[
                [10.0, 14.0],
                [8.0, 16.0],
                [5.0, 16.0],
                [3.0, 14.0],
                [3.0, 11.0],
                [8.0, 6.0],
                [11.0, 6.0],
                [13.0, 8.0],
            ]);
            line(&[
                [11.0, 16.0],
                [13.0, 18.0],
                [16.0, 18.0],
                [21.0, 13.0],
                [21.0, 10.0],
                [19.0, 8.0],
                [16.0, 8.0],
                [14.0, 10.0],
            ]);
            if matches!(icon, Icon::Unlink) {
                line(&[[3.0, 3.0], [21.0, 21.0]]);
            } else {
                line(&[[8.0, 16.0], [16.0, 8.0]]);
            }
        }
        Icon::Power => {
            painter.add(Shape::line(
                (0..=24)
                    .map(|i| {
                        let angle = (-60.0 + i as f32 * 12.5) * PI / 180.0;
                        point(12.0 + 9.0 * angle.cos(), 12.0 + 9.0 * angle.sin())
                    })
                    .collect(),
                stroke,
            ));
            line(&[[12.0, 2.0], [12.0, 12.0]]);
        }
        Icon::Return => {
            line(&[[20.0, 5.0], [20.0, 15.0], [4.0, 15.0]]);
            line(&[[9.0, 10.0], [4.0, 15.0], [9.0, 20.0]]);
        }
        Icon::Prompt => {
            line(&[[5.0, 5.0], [12.0, 12.0], [5.0, 19.0]]);
            line(&[[14.0, 19.0], [21.0, 19.0]]);
        }
        Icon::Terminal => {
            painter.rect_stroke(
                egui::Rect::from_min_max(point(2.0, 3.0), point(22.0, 21.0)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(&[[6.0, 8.0], [10.0, 12.0], [6.0, 16.0]]);
            line(&[[13.0, 16.0], [18.0, 16.0]]);
        }
        Icon::Layout => {
            painter.rect_stroke(
                egui::Rect::from_min_max(point(2.0, 3.0), point(22.0, 21.0)),
                3.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            line(&[[2.0, 8.0], [22.0, 8.0]]);
            line(&[[9.0, 8.0], [9.0, 21.0]]);
        }
        Icon::Summary => {
            line(&[[3.0, 6.0], [5.0, 8.0], [8.0, 4.0]]);
            line(&[[11.0, 6.0], [21.0, 6.0]]);
            line(&[[3.0, 13.0], [5.0, 15.0], [8.0, 11.0]]);
            line(&[[11.0, 13.0], [21.0, 13.0]]);
            line(&[[3.0, 20.0], [21.0, 20.0]]);
        }
        Icon::Sun => {
            painter.circle_stroke(center, size * 0.20, stroke);
            for i in 0..8 {
                let angle = i as f32 * PI / 4.0;
                painter.line_segment(
                    [
                        point(12.0 + 8.0 * angle.cos(), 12.0 + 8.0 * angle.sin()),
                        point(12.0 + 11.0 * angle.cos(), 12.0 + 11.0 * angle.sin()),
                    ],
                    stroke,
                );
            }
        }
        Icon::Moon => {
            line(&[
                [15.0, 3.0],
                [10.0, 4.0],
                [6.0, 7.0],
                [4.0, 11.0],
                [4.0, 15.0],
                [7.0, 19.0],
                [11.0, 21.0],
                [16.0, 20.0],
                [20.0, 16.0],
                [15.0, 16.0],
                [11.0, 13.0],
                [10.0, 9.0],
                [11.0, 5.0],
                [15.0, 3.0],
            ]);
        }
    }
}
