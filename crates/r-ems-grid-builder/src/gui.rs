//! ---
//! ems_section: "09-integration-interoperability"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Grid modelling helpers for partner integrations."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashMap;
use std::path::Path;

use egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use egui_extras::RetainedImage;

use crate::component::{ComponentKind, ComponentState, ComponentStatus};
use crate::icon_loader::get_icon_path;

const DEFAULT_ICON_SIZE: f32 = 64.0;
const BORDER_RADIUS: f32 = 8.0;

/// Component metadata used by the renderer.
#[derive(Debug, Clone)]
pub struct NodeComponent {
    pub state: ComponentState,
    pub position: Pos2,
    pub label: Option<String>,
}

impl NodeComponent {
    pub fn new(state: ComponentState, position: Pos2) -> Self {
        Self {
            state,
            position,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Icon cache and drawing helper for the visual grid builder.
#[derive(Default)]
pub struct IconRenderer {
    cache: HashMap<ComponentKind, RetainedImage>,
    padding: f32,
}

impl IconRenderer {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            padding: 4.0,
        }
    }

    fn icon(&mut self, kind: ComponentKind) -> Option<&RetainedImage> {
        if !self.cache.contains_key(&kind) {
            let path = get_icon_path(&kind);
            if let Ok(image) = RetainedImage::from_image_path(Path::new(&path)) {
                self.cache.insert(kind, image);
            }
        }
        self.cache.get(&kind)
    }

    /// Draw a component icon at the supplied position with zoom-aware scaling
    /// and status overlays.
    pub fn draw_component(&mut self, ui: &mut egui::Ui, component: &NodeComponent, zoom: f32) {
        let Some(icon) = self.icon(component.state.kind) else {
            return;
        };
        let zoom = zoom.max(0.2);
        let mut size = icon.size_vec2();
        if size == Vec2::ZERO {
            size = Vec2::splat(DEFAULT_ICON_SIZE);
        }
        let size = size * zoom;
        let rect = Rect::from_center_size(component.position, size);
        if !ui.is_rect_visible(rect.expand(2.0)) {
            return;
        }
        icon.paint_at(ui, rect);
        self.paint_status_overlay(ui, rect, component.state.status, zoom);
        if let Some(label) = &component.label {
            let text_pos = Pos2::new(rect.center().x, rect.bottom() + 4.0);
            ui.painter().text(
                text_pos,
                egui::Align2::CENTER_TOP,
                label,
                egui::FontId::proportional(12.0 * zoom.clamp(0.75, 1.5)),
                Color32::WHITE,
            );
        }
    }

    fn paint_status_overlay(&self, ui: &egui::Ui, rect: Rect, status: ComponentStatus, zoom: f32) {
        let stroke_width = (2.0 * zoom).clamp(1.0, 4.0);
        let (stroke_color, overlay) = match status {
            ComponentStatus::Healthy => (Color32::from_rgb(67, 160, 71), None),
            ComponentStatus::Fault => (
                Color32::from_rgb(211, 47, 47),
                Some(Color32::from_rgba_premultiplied(211, 47, 47, 40)),
            ),
            ComponentStatus::Offline => (
                Color32::from_rgb(120, 144, 156),
                Some(Color32::from_rgba_premultiplied(38, 50, 56, 160)),
            ),
        };
        if let Some(color) = overlay {
            ui.painter().rect_filled(rect, BORDER_RADIUS, color);
        }
        ui.painter().rect_stroke(
            rect.expand(self.padding * zoom),
            BORDER_RADIUS,
            Stroke::new(stroke_width, stroke_color),
        );
    }
}
