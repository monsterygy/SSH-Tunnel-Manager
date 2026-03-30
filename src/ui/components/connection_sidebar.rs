use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{self, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::label;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{h_flex, v_flex};
use rust_i18n::t;
use std::sync::Arc;

use ssh_tunnel_manager::models::auth::AuthMethod;
use ssh_tunnel_manager::state::AppState;

use super::theme::AppColors;

/// Render the left sidebar panel with connection list.
pub fn render_connection_sidebar(
    app_state: &Arc<AppState>,
    search_input: &Entity<InputState>,
    is_dark: bool,
) -> Div {
    let panel_bg = AppColors::page_bg(is_dark);
    let border = AppColors::border_color(is_dark);
    let text = AppColors::primary_text(is_dark);
    let muted = AppColors::secondary_text(is_dark);
    let card_bg = AppColors::card_bg(is_dark);
    let accent = AppColors::accent_color(is_dark);
    let success = AppColors::success_color();
    let danger = AppColors::danger_color();
    let inactive_dot = AppColors::inactive_dot(is_dark);

    let danger_bg = if is_dark {
        hsla(0.0, 0.40, 0.20, 1.0)
    } else {
        hsla(0.0, 0.86, 0.97, 1.0)
    };
    let danger_border = if is_dark {
        hsla(0.0, 0.70, 0.50, 1.0)
    } else {
        hsla(0.0, 0.92, 0.87, 1.0)
    };
    let danger_text = if is_dark {
        hsla(0.0, 0.75, 0.70, 1.0)
    } else {
        hsla(0.0, 0.70, 0.35, 1.0)
    };

    // Get filter text and connections
    let filter_text = if let Ok(ui_state) = app_state.ui_state.try_read() {
        ui_state.filter_text.clone()
    } else {
        String::new()
    };

    let all_connections = if let Ok(conns) = app_state.connections.try_read() {
        conns.clone()
    } else {
        vec![]
    };

    // Get active sessions to show connection status
    let active_connection_ids: Vec<uuid::Uuid> = if let Ok(sessions) = app_state.sessions.try_read()
    {
        sessions.iter().map(|s| s.connection_id).collect()
    } else {
        vec![]
    };

    // Get confirm delete state
    let confirm_delete_id = if let Ok(ui_state) = app_state.ui_state.try_read() {
        ui_state.confirm_delete_id
    } else {
        None
    };

    // Filter connections based on search
    let connections: Vec<_> = if filter_text.is_empty() {
        all_connections.clone()
    } else {
        let filter_lower = filter_text.to_lowercase();
        all_connections
            .iter()
            .filter(|c| {
                c.name.to_lowercase().contains(&filter_lower)
                    || c.host.to_lowercase().contains(&filter_lower)
                    || c.username.to_lowercase().contains(&filter_lower)
            })
            .cloned()
            .collect()
    };

    let selected_id = if let Ok(state) = app_state.selected_connection_id.try_read() {
        *state
    } else {
        None
    };

    v_flex()
        .w(px(280.0))
        .h_full()
        .bg(panel_bg)
        .border_r_1()
        .border_color(border)
        // Header: "Connections" + count badge + search
        .child(
            v_flex()
                .flex_shrink_0()
                .p_4()
                .gap_3()
                .border_b_1()
                .border_color(border)
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(
                            label::Label::new(t!("connection.connections").to_string())
                                .text_size(rems(0.95))
                                .font_weight(FontWeight::BOLD)
                                .text_color(text),
                        )
                        .child(
                            div()
                                .px_2()
                                .py(px(2.0))
                                .bg(accent.opacity(0.12))
                                .rounded_lg()
                                .text_xs()
                                .text_color(accent)
                                .child(format!("{}", all_connections.len())),
                        ),
                )
                .child(Input::new(search_input).cleanable(true)),
        )
        // Connection list (scrollable)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .child(
                    v_flex()
                        .px_3()
                        .py_2()
                        .gap_1()
                        .when(connections.is_empty(), |this| {
                            this.child(
                                v_flex()
                                    .p_4()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(muted)
                                            .text_center()
                                            .child(if filter_text.is_empty() {
                                                t!("connection.no_connections").to_string()
                                            } else {
                                                t!("connection.no_matching").to_string()
                                            }),
                                    )
                                    .when(filter_text.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(muted)
                                                .text_center()
                                                .mt_2()
                                                .child(
                                                    t!("connection.click_new").to_string(),
                                                ),
                                        )
                                    }),
                            )
                        })
                        .children(connections.iter().map(|conn| {
                            render_connection_list_item(
                                app_state,
                                conn,
                                selected_id,
                                &active_connection_ids,
                                is_dark,
                                card_bg,
                                border,
                                text,
                                muted,
                                accent,
                                success,
                                danger,
                                inactive_dot,
                            )
                        })),
                ),
        )
        // Delete confirmation dialog
        .when(confirm_delete_id.is_some(), |this| {
            let app_state = app_state.clone();
            let app_state_cancel = app_state.clone();
            let conn_name = if let Some(id) = confirm_delete_id {
                all_connections
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default()
            } else {
                String::new()
            };

            this.child(
                div()
                    .flex_shrink_0()
                    .p_3()
                    .bg(danger_bg)
                    .border_t_1()
                    .border_color(danger_border)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div().text_sm().text_color(danger_text).child(
                                    t!("messages.delete_confirm_title", "name" => conn_name.as_str())
                                        .to_string(),
                                ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        button::Button::new("confirm_delete")
                                            .danger()
                                            .compact()
                                            .label(
                                                t!("actions.confirm_delete").to_string(),
                                            )
                                            .on_click(move |_, _, _| {
                                                let app_state = app_state.clone();
                                                tokio::spawn(async move {
                                                    if let Err(e) =
                                                        app_state.confirm_delete().await
                                                    {
                                                        tracing::error!(
                                                            "Failed to delete: {}",
                                                            e
                                                        );
                                                    }
                                                });
                                            }),
                                    )
                                    .child(
                                        button::Button::new("cancel_delete")
                                            .compact()
                                            .label(t!("actions.cancel").to_string())
                                            .on_click(move |_, _, _| {
                                                let app_state = app_state_cancel.clone();
                                                tokio::spawn(async move {
                                                    app_state.hide_delete_confirm().await;
                                                });
                                            }),
                                    ),
                            ),
                    ),
            )
        })
        // Bottom bar: only "New Connection" button
        .child(
            h_flex()
                .flex_shrink_0()
                .gap_2()
                .h(px(48.0))
                .px_4()
                .items_center()
                .border_t_1()
                .border_color(border)
                .bg(card_bg)
                .child({
                    let app_state = app_state.clone();
                    button::Button::new("new_left")
                        .primary()
                        .label(t!("actions.new").to_string())
                        .on_click(move |_, _, _| {
                            let app_state = app_state.clone();
                            tokio::spawn(async move {
                                app_state.clear_selection_for_new().await;
                            });
                        })
                }),
        )
}

/// Render a single connection list item card.
#[allow(clippy::too_many_arguments)]
fn render_connection_list_item(
    app_state: &Arc<AppState>,
    conn: &ssh_tunnel_manager::models::connection::SshConnection,
    selected_id: Option<uuid::Uuid>,
    active_connection_ids: &[uuid::Uuid],
    is_dark: bool,
    card_bg: Hsla,
    border: Hsla,
    text: Hsla,
    muted: Hsla,
    accent: Hsla,
    success: Hsla,
    danger: Hsla,
    _inactive_dot: Hsla,
) -> Div {
    let is_selected = selected_id == Some(conn.id);
    let conn_id = conn.id;
    let app_state_select = app_state.clone();
    let app_state_connect = app_state.clone();
    let is_connected = active_connection_ids.contains(&conn.id);
    let conn_clone = conn.clone();

    // Build forwarding summary text
    let fwd_summary = if conn.forwarding_configs.is_empty() {
        String::new()
    } else {
        match &conn.forwarding_configs[0] {
            ssh_tunnel_manager::models::forwarding::ForwardingConfig::Local(fwd) => {
                format!("Local {}>{}", fwd.local_port, fwd.remote_port)
            }
            ssh_tunnel_manager::models::forwarding::ForwardingConfig::Remote(fwd) => {
                format!("Remote {}>{}", fwd.remote_port, fwd.local_port)
            }
            ssh_tunnel_manager::models::forwarding::ForwardingConfig::Dynamic(fwd) => {
                format!("SOCKS5 :{}", fwd.local_port)
            }
        }
    };

    div()
        .w_full()
        .px_2()
        .py_2()
        .rounded_lg()
        .bg(if is_selected {
            AppColors::selected_bg(is_dark)
        } else {
            card_bg
        })
        // Selected: 3px left border in accent, BOLD name
        .when(is_selected, |this| {
            this.border_l(px(3.0)).border_color(accent)
        })
        .when(!is_selected, |this| this.border_1().border_color(border))
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
            let app_state = app_state_select.clone();
            tokio::spawn(async move {
                app_state.select_and_load_connection(conn_id).await;
            });
        })
        .child(
            v_flex()
                .gap_1()
                // Row 1: Name + status dot
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(if is_selected {
                                    FontWeight::BOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(text)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(conn.name.clone()),
                        )
                        .child(div().flex_shrink_0().size(px(8.0)).rounded_full().bg(
                            if is_connected {
                                success
                            } else {
                                AppColors::disconnected_dot(is_dark)
                            },
                        )),
                )
                // Row 2: user@host:port
                .child(
                    div()
                        .text_xs()
                        .text_color(muted)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(format!("{}@{}:{}", conn.username, conn.host, conn.port)),
                )
                // Row 3: Forwarding summary
                .when(!fwd_summary.is_empty(), |this| {
                    this.child(div().text_xs().text_color(muted).child(fwd_summary.clone()))
                })
                // Action area: button-style Connect/Disconnect
                .child(h_flex().mt_1().child(if is_connected {
                    let app_state_disc = app_state.clone();
                    let sess_conn_id = conn.id;
                    let btn_id: &'static str =
                        Box::leak(format!("dc_{}", conn_id).into_boxed_str());
                    div()
                        .id(btn_id)
                        .cursor_pointer()
                        .px_2()
                        .py(px(2.0))
                        .rounded_lg()
                        .bg(danger.opacity(0.10))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(danger)
                        .hover(|s| s.bg(danger.opacity(0.18)))
                        .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
                            let app_state = app_state_disc.clone();
                            tokio::spawn(async move {
                                if let Ok(sessions) = app_state.sessions.try_read() {
                                    if let Some(session) =
                                        sessions.iter().find(|s| s.connection_id == sess_conn_id)
                                    {
                                        let sid = session.id;
                                        drop(sessions);
                                        let _ = app_state.disconnect_session(sid).await;
                                    }
                                }
                            });
                        })
                        .child(t!("actions.disconnect").to_string())
                } else {
                    let btn_id: &'static str =
                        Box::leak(format!("qc_{}", conn_id).into_boxed_str());
                    div()
                        .id(btn_id)
                        .cursor_pointer()
                        .px_2()
                        .py(px(2.0))
                        .rounded_lg()
                        .bg(accent.opacity(0.10))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(accent)
                        .hover(|s| s.bg(accent.opacity(0.18)))
                        .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
                            let app_state = app_state_connect.clone();
                            let conn = conn_clone.clone();
                            tokio::spawn(async move {
                                match &conn.auth_method {
                                    AuthMethod::Password => {
                                        app_state.show_password_input(conn.id).await;
                                    }
                                    AuthMethod::PublicKey {
                                        passphrase_required,
                                        ..
                                    } => {
                                        if *passphrase_required {
                                            app_state.show_password_input(conn.id).await;
                                        } else {
                                            let _ = app_state.connect_session(conn.id, None).await;
                                        }
                                    }
                                }
                            });
                        })
                        .child(t!("actions.connect").to_string())
                })),
        )
}
