use gpui::*;
use gpui_component::label;
use gpui_component::h_flex;
use rust_i18n::t;
use std::sync::Arc;

use ssh_tunnel_manager::state::AppState;

use super::theme::AppColors;

/// Render the top header bar with app title, language toggle, and theme toggle.
pub fn render_top_bar(app_state: &Arc<AppState>, is_dark: bool) -> Div {
    let header_bg = AppColors::card_bg(is_dark);
    let text = AppColors::primary_text(is_dark);
    let border = AppColors::border_color(is_dark);
    let accent = AppColors::accent_color(is_dark);

    let (dark_mode, language) = if let Ok(ui_state) = app_state.ui_state.try_read() {
        (ui_state.dark_mode, ui_state.language.clone())
    } else {
        (false, "en".to_string())
    };

    let app_state_lang = app_state.clone();
    let app_state_theme = app_state.clone();

    h_flex()
        .items_center()
        .justify_between()
        .px_4()
        .py_2()
        .bg(header_bg)
        .border_b_1()
        .border_color(border)
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child("SSH"),
                )
                .child(
                    label::Label::new(t!("app.title").to_string())
                        .text_size(rems(1.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_1()
                // Language toggle — icon-style button with bg
                .child({
                    let btn_bg = if is_dark {
                        hsla(0.0, 0.0, 0.20, 1.0)
                    } else {
                        hsla(0.0, 0.0, 0.94, 1.0)
                    };
                    let muted = AppColors::secondary_text(is_dark);
                    div()
                        .id("lang-toggle")
                        .cursor_pointer()
                        .px_2()
                        .py(px(3.0))
                        .rounded_lg()
                        .bg(btn_bg)
                        .border_1()
                        .border_color(border)
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(muted)
                        .hover(|s| s.bg(accent.opacity(0.10)))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            move |_, window, _| {
                                if let Ok(mut ui_state) = app_state_lang.ui_state.try_write() {
                                    ui_state.language = if ui_state.language == "zh-CN" {
                                        "en".to_string()
                                    } else {
                                        "zh-CN".to_string()
                                    };
                                    crate::utils::i18n::change_language(&ui_state.language);
                                }
                                window.refresh();
                            },
                        )
                        .child(if language == "zh-CN" { "EN" } else { "中" })
                })
                // Theme toggle — icon-style button with bg
                .child({
                    let btn_bg = if is_dark {
                        hsla(0.0, 0.0, 0.20, 1.0)
                    } else {
                        hsla(0.0, 0.0, 0.94, 1.0)
                    };
                    let muted = AppColors::secondary_text(is_dark);
                    div()
                        .id("theme-toggle")
                        .cursor_pointer()
                        .px_2()
                        .py(px(3.0))
                        .rounded_lg()
                        .bg(btn_bg)
                        .border_1()
                        .border_color(border)
                        .text_xs()
                        .text_color(muted)
                        .hover(|s| s.bg(accent.opacity(0.10)))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            move |_, window, cx| {
                                let is_dark =
                                    if let Ok(mut ui_state) = app_state_theme.ui_state.try_write() {
                                        ui_state.dark_mode = !ui_state.dark_mode;
                                        ui_state.dark_mode
                                    } else {
                                        return;
                                    };
                                use gpui_component::theme::{Theme, ThemeMode};
                                Theme::change(
                                    if is_dark {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    },
                                    Some(window),
                                    cx,
                                );
                            },
                        )
                        .child(if dark_mode { "☀️" } else { "🌙" })
                }),
        )
}
