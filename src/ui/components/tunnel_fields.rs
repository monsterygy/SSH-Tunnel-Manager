use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputState};
use gpui_component::label;
use gpui_component::{h_flex, v_flex};
use rust_i18n::t;
use std::sync::Arc;

use ssh_tunnel_manager::state::{AppState, ConnectionFormData};

use super::section_card::section_card;
use super::theme::AppColors;

/// Render a tunnel mode radio button.
fn render_mode_radio(
    app_state: &Arc<AppState>,
    label_text: &str,
    selected: bool,
    mode: &str,
    border_color: Hsla,
    text_color: Hsla,
    accent_color: Hsla,
    muted_color: Hsla,
) -> impl IntoElement {
    let app_state = app_state.clone();
    let mode = mode.to_string();

    div()
        .cursor_pointer()
        .px_3()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(if selected { accent_color } else { border_color })
        .bg(if selected {
            accent_color.opacity(0.08)
        } else {
            gpui::transparent_black()
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
            let app_state = app_state.clone();
            let mode = mode.clone();
            tokio::spawn(async move {
                app_state.update_form_field("forwarding_type", mode).await;
            });
        })
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(14.0))
                        .rounded_full()
                        .border_2()
                        .border_color(if selected { accent_color } else { border_color })
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(selected, |this| {
                            this.child(div().size(px(6.0)).rounded_full().bg(accent_color))
                        }),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(if selected { text_color } else { muted_color })
                        .child(label_text.to_string()),
                ),
        )
}

/// Render the tunnel mode card with mode selector and forwarding fields.
pub fn render_tunnel_card(
    app_state: &Arc<AppState>,
    form_data: &ConnectionFormData,
    local_port_input: &Entity<InputState>,
    remote_host_input: &Entity<InputState>,
    remote_port_input: &Entity<InputState>,
    bind_address_input: &Entity<InputState>,
    is_dark: bool,
) -> Div {
    let border = AppColors::border_color(is_dark);
    let text = AppColors::primary_text(is_dark);
    let muted = AppColors::secondary_text(is_dark);
    let accent = AppColors::accent_color(is_dark);

    let is_dynamic = form_data.forwarding_type == "dynamic";
    let is_remote = form_data.forwarding_type == "remote";

    section_card(&t!("connection.tunnel_mode").to_string(), is_dark)
        // Mode selector
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(render_mode_radio(
                    app_state,
                    &format!("{} (-L)", t!("forwarding.local")),
                    form_data.forwarding_type == "local",
                    "local",
                    border,
                    text,
                    accent,
                    muted,
                ))
                .child(render_mode_radio(
                    app_state,
                    &format!("{} (-R)", t!("forwarding.remote")),
                    form_data.forwarding_type == "remote",
                    "remote",
                    border,
                    text,
                    accent,
                    muted,
                ))
                .child(render_mode_radio(
                    app_state,
                    &format!("{} (-D)", t!("forwarding.dynamic")),
                    form_data.forwarding_type == "dynamic",
                    "dynamic",
                    border,
                    text,
                    accent,
                    muted,
                )),
        )
        // One-line mode description
        .child(
            div().text_sm().text_color(muted).child(
                match form_data.forwarding_type.as_str() {
                    "local" => t!("connection.local_mode_hint").to_string(),
                    "remote" => t!("connection.remote_mode_hint").to_string(),
                    "dynamic" => t!("connection.dynamic_mode_hint").to_string(),
                    _ => String::new(),
                },
            ),
        )
        // Forwarding fields
        .child(
            v_flex()
                .gap_3()
                // Bind Address / Local Port row (always shown)
                .child(
                    h_flex()
                        .gap_3()
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1()
                                .child(
                                    label::Label::new(
                                        t!("forwarding.bind_address").to_string(),
                                    )
                                    .text_size(rems(0.8))
                                    .text_color(muted),
                                )
                                .child(Input::new(bind_address_input).cleanable(true)),
                        )
                        .child(
                            v_flex()
                                .w(px(120.0))
                                .gap_1()
                                .child(
                                    label::Label::new(if is_dynamic {
                                        "SOCKS Port".to_string()
                                    } else {
                                        t!("connection.local_port").to_string()
                                    })
                                    .text_size(rems(0.8))
                                    .text_color(muted),
                                )
                                .child(Input::new(local_port_input).cleanable(true)),
                        ),
                )
                // Remote destination (Local and Remote modes only)
                .when(!is_dynamic, |this| {
                    this.child(
                        h_flex()
                            .gap_3()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        label::Label::new(if is_remote {
                                            t!("connection.local_host").to_string()
                                        } else {
                                            t!("forwarding.remote_host").to_string()
                                        })
                                        .text_size(rems(0.8))
                                        .text_color(muted),
                                    )
                                    .child(Input::new(remote_host_input).cleanable(true)),
                            )
                            .child(
                                v_flex()
                                    .w(px(120.0))
                                    .gap_1()
                                    .child(
                                        label::Label::new(
                                            t!("connection.remote_port").to_string(),
                                        )
                                        .text_size(rems(0.8))
                                        .text_color(muted),
                                    )
                                    .child(Input::new(remote_port_input).cleanable(true)),
                            ),
                    )
                })
                // Dynamic SOCKS hint
                .when(is_dynamic, |this| {
                    this.child(
                        div()
                            .text_sm()
                            .text_color(muted)
                            .child(t!("connection.socks5_hint").to_string()),
                    )
                }),
        )
}
