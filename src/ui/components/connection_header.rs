use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{self, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::label;
use gpui_component::{h_flex, v_flex};
use rust_i18n::t;
use std::sync::Arc;

use ssh_tunnel_manager::models::auth::AuthMethod;
use ssh_tunnel_manager::state::{AppState, ConnectionFormData, ErrorSeverity};

use super::theme::AppColors;

/// Render the right panel header with connection name, status badge, and action buttons.
pub fn render_connection_header(
    app_state: &Arc<AppState>,
    form_data: &ConnectionFormData,
    editing_id: Option<uuid::Uuid>,
    is_connecting: bool,
    is_this_connected: bool,
    is_dark: bool,
) -> Div {
    let card_bg = AppColors::card_bg(is_dark);
    let border = AppColors::border_color(is_dark);
    let text = AppColors::primary_text(is_dark);
    let muted = AppColors::secondary_text(is_dark);

    let is_editing = editing_id.is_some();

    // Status badge using unified 4-state system
    let (status_bg, status_text_color, status_label) = if is_connecting {
        let (bg, tc, _lbl) = AppColors::status_connecting(is_dark);
        (bg, tc, t!("actions.connecting").to_string())
    } else if is_this_connected {
        let (bg, tc, _lbl) = AppColors::status_connected(is_dark);
        (bg, tc, t!("status.connected").to_string())
    } else {
        let (bg, tc, _lbl) = AppColors::status_disconnected(is_dark);
        (bg, tc, t!("connection.not_connected").to_string())
    };

    h_flex()
        .flex_shrink_0()
        .px_4()
        .py_3()
        .bg(card_bg)
        .border_b_1()
        .border_color(border)
        .items_center()
        .justify_between()
        // Left: name + subtitle (user@host:port · status)
        .child(
            h_flex()
                .items_center()
                .gap_3()
                .child(
                    v_flex()
                        .child(
                            label::Label::new(if form_data.name.is_empty() {
                                t!("app.new_connection").to_string()
                            } else {
                                form_data.name.clone()
                            })
                            .text_size(rems(1.2))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                // user@host:port
                                .when(
                                    !form_data.host.is_empty() || !form_data.username.is_empty(),
                                    |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(format!(
                                                    "{}@{}:{}",
                                                    if form_data.username.is_empty() { "-" } else { &form_data.username },
                                                    if form_data.host.is_empty() { "-" } else { &form_data.host },
                                                    form_data.port,
                                                )),
                                        )
                                    },
                                )
                                // separator dot
                                .when(
                                    !form_data.host.is_empty() || !form_data.username.is_empty(),
                                    |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .child("·"),
                                        )
                                    },
                                )
                                // Status badge — solid bg + contrasting text
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(2.0))
                                        .bg(status_bg)
                                        .rounded_lg()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(status_text_color)
                                        .child(status_label),
                                ),
                        ),
                ),
        )
        // Right: Template (new only) + Save + Connect/Disconnect + "⋯" more menu
        .child(
            h_flex()
                .gap_2()
                .items_center()
                // Template button (new connections only)
                .when(!is_editing, |this| {
                    let app_state = app_state.clone();
                    this.child(
                        div()
                            .id("template_dropdown")
                            .cursor_pointer()
                            .px_2()
                            .py(px(2.0))
                            .bg(if is_dark {
                                hsla(0.0, 0.0, 0.22, 1.0)
                            } else {
                                hsla(0.0, 0.0, 0.94, 1.0)
                            })
                            .rounded_lg()
                            .text_xs()
                            .text_color(muted)
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                move |_, _, _| {
                                    let app_state = app_state.clone();
                                    tokio::spawn(async move {
                                        app_state.toggle_templates().await;
                                    });
                                },
                            )
                            .child(t!("app.templates").to_string()),
                    )
                })
                // Save button (accent/primary — always accent since we don't track dirty state yet)
                .child({
                    let app_state = app_state.clone();
                    button::Button::new("save_btn")
                        .primary()
                        .label(t!("actions.save").to_string())
                        .on_click(move |_, _, _| {
                            let app_state = app_state.clone();
                            tokio::spawn(async move {
                                match app_state.save_connection_from_form().await {
                                    Ok(connection_id) => {
                                        tracing::info!("Connection saved: {}", connection_id);
                                        app_state
                                            .show_success(
                                                t!("messages.connection_saved").to_string(),
                                            )
                                            .await;
                                        app_state
                                            .select_and_load_connection(connection_id)
                                            .await;
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to save connection: {}", e);
                                        app_state
                                            .show_error(
                                                t!("messages.save_failed", "reason" => e.to_string())
                                                    .to_string(),
                                                ErrorSeverity::Error,
                                            )
                                            .await;
                                    }
                                }
                            });
                        })
                })
                // Connect/Disconnect button (only for saved connections)
                .when(is_editing, |this| {
                    if is_this_connected {
                        let app_state = app_state.clone();
                        let conn_id = editing_id.unwrap();
                        this.child(
                            button::Button::new("disconnect_btn")
                                .danger()
                                .label(t!("actions.disconnect").to_string())
                                .on_click(move |_, _, _| {
                                    let app_state = app_state.clone();
                                    tokio::spawn(async move {
                                        if let Ok(sessions) = app_state.sessions.try_read() {
                                            if let Some(session) =
                                                sessions.iter().find(|s| s.connection_id == conn_id)
                                            {
                                                let sid = session.id;
                                                drop(sessions);
                                                let _ =
                                                    app_state.disconnect_session(sid).await;
                                            }
                                        }
                                    });
                                }),
                        )
                    } else {
                        let app_state = app_state.clone();
                        let conn_id = editing_id.unwrap();
                        this.child(
                            button::Button::new("connect_btn")
                                .success()
                                .label(t!("actions.connect").to_string())
                                .on_click(move |_, _, _| {
                                    let app_state = app_state.clone();
                                    tokio::spawn(async move {
                                        if let Some(conn) =
                                            app_state.get_connection(conn_id).await
                                        {
                                            match &conn.auth_method {
                                                AuthMethod::Password => {
                                                    app_state
                                                        .show_password_input(conn_id)
                                                        .await;
                                                }
                                                AuthMethod::PublicKey {
                                                    passphrase_required,
                                                    ..
                                                } => {
                                                    if *passphrase_required {
                                                        app_state
                                                            .show_password_input(conn_id)
                                                            .await;
                                                    } else {
                                                        match app_state
                                                            .connect_session(conn_id, None)
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                app_state
                                                                    .show_success(
                                                                        t!("messages.connection_success")
                                                                            .to_string(),
                                                                    )
                                                                    .await;
                                                            }
                                                            Err(e) => {
                                                                app_state
                                                                    .show_error(
                                                                        t!("messages.connection_failed", "reason" => e.to_string())
                                                                            .to_string(),
                                                                        ErrorSeverity::Error,
                                                                    )
                                                                    .await;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }),
                        )
                    }
                })
                // "⋯" more menu — contains Delete (danger action hidden from main flow)
                .when(is_editing, |this| {
                    let app_state = app_state.clone();
                    let conn_id = editing_id.unwrap();
                    this.child(
                        div()
                            .id("more_menu_btn")
                            .cursor_pointer()
                            .size(px(28.0))
                            .rounded_lg()
                            .border_1()
                            .border_color(border)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(muted)
                            .hover(|s| s.bg(AppColors::page_bg(is_dark)))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                move |_, _, _| {
                                    let app_state = app_state.clone();
                                    tokio::spawn(async move {
                                        app_state.show_delete_confirm(conn_id).await;
                                    });
                                },
                            )
                            .child("⋯"),
                    )
                }),
        )
}

/// Render the password input section (shown when authentication requires a password).
pub fn render_password_section(
    app_state: &Arc<AppState>,
    password_input: &Entity<InputState>,
    password_input_for: uuid::Uuid,
    is_dark: bool,
) -> Div {
    let warning_bg = if is_dark {
        hsla(38.0 / 360.0, 0.40, 0.20, 1.0)
    } else {
        hsla(45.0 / 360.0, 0.93, 0.89, 1.0)
    };
    let warning_border = AppColors::warning_color();
    let warning_text = if is_dark {
        hsla(38.0 / 360.0, 0.80, 0.70, 1.0)
    } else {
        hsla(28.0 / 360.0, 0.80, 0.31, 1.0)
    };

    let conn_id = password_input_for;
    let app_state_submit = app_state.clone();
    let app_state_cancel = app_state.clone();

    div()
        .flex_shrink_0()
        .p_4()
        .bg(warning_bg)
        .border_b_1()
        .border_color(warning_border)
        .child(
            v_flex()
                .gap_3()
                .child(
                    label::Label::new(t!("connection.enter_password").to_string())
                        .text_size(rems(0.95))
                        .text_color(warning_text),
                )
                .child(
                    h_flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(password_input).cleanable(true)),
                        )
                        .child(
                            button::Button::new("submit_password")
                                .success()
                                .label(t!("actions.connect").to_string())
                                .on_click(move |_, _, _| {
                                    let app_state = app_state_submit.clone();
                                    tokio::spawn(async move {
                                        let password = app_state.get_password_value().await;
                                        app_state.hide_password_input().await;
                                        match app_state
                                            .connect_session(conn_id, Some(password))
                                            .await
                                        {
                                            Ok(_) => {
                                                app_state
                                                    .show_success(
                                                        t!("messages.connection_success")
                                                            .to_string(),
                                                    )
                                                    .await;
                                            }
                                            Err(e) => {
                                                app_state
                                                    .show_error(
                                                        t!("messages.connection_failed", "reason" => e.to_string())
                                                            .to_string(),
                                                        ErrorSeverity::Error,
                                                    )
                                                    .await;
                                            }
                                        }
                                    });
                                }),
                        )
                        .child(
                            button::Button::new("cancel_password")
                                .label(t!("actions.cancel").to_string())
                                .on_click(move |_, _, _| {
                                    let app_state = app_state_cancel.clone();
                                    tokio::spawn(async move {
                                        app_state.hide_password_input().await;
                                    });
                                }),
                        ),
                ),
        )
}
