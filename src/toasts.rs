//! M3 snackbars at the bottom of the window.

use crate::theme::M3Palette;
use crate::widgets;
use egui::{Align2, CornerRadius, Frame, Id, Margin, RichText};
use egui_material_icons::icons::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const MAX_VISIBLE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct Toast<A> {
    id: u64,
    pub text: String,
    pub kind: ToastKind,
    /// An optional button (label, action returned by [`Toasts::show`]).
    pub action: Option<(String, A)>,
    shown: Instant,
    /// Hovering pushes this back so the toast can be read.
    expires: Instant,
}

impl<A> Toast<A> {
    pub fn new(text: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            text: text.into(),
            kind,
            action: None,
            shown: Instant::now(),
            expires: Instant::now()
                + Duration::from_secs(if kind == ToastKind::Error { 7 } else { 4 }),
        }
    }
}

/// The toast stack. `A` is the app's action type, returned when a toast's
/// button is pressed.
pub struct Toasts<A> {
    list: Vec<Toast<A>>,
}

impl<A> Default for Toasts<A> {
    fn default() -> Self {
        Self { list: Vec::new() }
    }
}

impl<A: Clone> Toasts<A> {
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(Toast::new(text, ToastKind::Info));
    }

    pub fn success(&mut self, text: impl Into<String>) {
        self.push(Toast::new(text, ToastKind::Success));
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(Toast::new(text, ToastKind::Error));
    }

    pub fn push(&mut self, mut toast: Toast<A>) {
        if toast.action.is_some() {
            toast.expires = toast.expires.max(Instant::now() + Duration::from_secs(7));
        }
        // Collapse exact repeats instead of stacking them.
        self.list.retain(|t| t.text != toast.text);
        self.list.push(toast);
        if self.list.len() > MAX_VISIBLE {
            self.list.remove(0);
        }
    }

    /// Draw the stack. `bottom` is the gap above the window's bottom edge
    /// (leave room for a status or player bar). Returns the actions of
    /// toast buttons pressed this frame.
    pub fn show(&mut self, ctx: &egui::Context, p: &M3Palette, bottom: f32) -> Vec<A> {
    let now = Instant::now();
    let mut dismiss = Vec::new();
    let mut fire = Vec::new();

    egui::Area::new(Id::unique("toasts"))
        .anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -bottom))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for toast in &mut self.list {
                let appear = ((now - toast.shown).as_secs_f32() / 0.15).min(1.0);
                let resp = Frame::new()
                    .fill(p.inverse_surface)
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin {
                        left: 16,
                        right: 8,
                        top: 6,
                        bottom: 6,
                    })
                    .shadow(ui.visuals().popup_shadow)
                    .multiply_with_opacity(appear)
                    .show(ui, |ui| {
                        ui.set_min_width(320.0);
                        ui.set_max_width(560.0);
                        ui.horizontal(|ui| {
                            let (icon, color) = match toast.kind {
                                ToastKind::Info => (ICON_INFO, p.inverse_on_surface),
                                ToastKind::Success => (ICON_CHECK_CIRCLE, p.inverse_primary),
                                ToastKind::Error => (ICON_ERROR, p.error_container),
                            };
                            widgets::icon(ui, icon, 20.0, color);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&toast.text).color(p.inverse_on_surface),
                                )
                                .wrap(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let close = ui.add(
                                        egui::Button::new(
                                            widgets::icon_text(ICON_CLOSE, 18.0)
                                                .color(p.inverse_on_surface),
                                        )
                                        .frame(false),
                                    );
                                    if close.clicked() {
                                        dismiss.push(toast.id);
                                    }
                                    if let Some((label, action)) = &toast.action {
                                        let b = ui.add(
                                            egui::Button::new(
                                                RichText::new(label)
                                                    .color(p.inverse_primary)
                                                    .size(14.0),
                                            )
                                            .frame(false),
                                        );
                                        if b.clicked() {
                                            fire.push(action.clone());
                                            dismiss.push(toast.id);
                                        }
                                    }
                                },
                            );
                        });
                    });
                if resp.response.contains_pointer() {
                    toast.expires = toast.expires.max(now + Duration::from_secs(2));
                } else if now >= toast.expires {
                    dismiss.push(toast.id);
                }
            }
        });

    self.list.retain(|t| !dismiss.contains(&t.id));
    if !self.list.is_empty() {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    fire
    }
}
