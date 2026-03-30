use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::*;
use rust_i18n::t;
use std::cell::Cell;
use std::sync::Arc;

use ssh_tunnel_manager::state::{AppState, ConnectionFormData, ErrorSeverity};

use super::components::{
    AppColors, render_connection_header, render_connection_sidebar, render_password_section,
    render_top_bar, render_tunnel_card, section_card,
};

/// Main application window with editable form inputs
pub struct SshTunnelApp {
    app_state: Arc<AppState>,
    // Sidebar inputs
    search_input: Entity<InputState>,
    password_input: Entity<InputState>,
    // Form input states
    name_input: Entity<InputState>,
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    username_input: Entity<InputState>,
    private_key_path_input: Entity<InputState>,
    local_port_input: Entity<InputState>,
    remote_host_input: Entity<InputState>,
    remote_port_input: Entity<InputState>,
    bind_address_input: Entity<InputState>,
    // UI-only state
    show_advanced: Cell<bool>,
}

impl SshTunnelApp {
    // ── Sync ──────────────────────────────────────────────────────────

    /// Sync form_data to Input components
    fn sync_form_to_inputs(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(ui_state) = self.app_state.ui_state.try_read() {
            let form_data = &ui_state.form_data;

            self.name_input.update(cx, |state, cx| {
                state.set_value(&form_data.name, window, cx);
            });
            self.host_input.update(cx, |state, cx| {
                state.set_value(&form_data.host, window, cx);
            });
            self.port_input.update(cx, |state, cx| {
                state.set_value(&form_data.port, window, cx);
            });
            self.username_input.update(cx, |state, cx| {
                state.set_value(&form_data.username, window, cx);
            });
            self.private_key_path_input.update(cx, |state, cx| {
                state.set_value(&form_data.private_key_path, window, cx);
            });
            self.local_port_input.update(cx, |state, cx| {
                state.set_value(&form_data.local_port, window, cx);
            });
            self.remote_host_input.update(cx, |state, cx| {
                state.set_value(&form_data.remote_host, window, cx);
            });
            self.remote_port_input.update(cx, |state, cx| {
                state.set_value(&form_data.remote_port, window, cx);
            });
            self.bind_address_input.update(cx, |state, cx| {
                state.set_value(&form_data.bind_address, window, cx);
            });

            // Clear password input when not showing password prompt
            if ui_state.password_input_for.is_none() {
                self.password_input.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                });
            }

            // Update search placeholder for i18n
            self.search_input.update(cx, |state, cx| {
                state.set_placeholder(&t!("search.placeholder").to_string(), window, cx);
            });

            // Update password input placeholder for i18n
            self.password_input.update(cx, |state, cx| {
                state.set_placeholder(&t!("connection.enter_password").to_string(), window, cx);
            });
        }
    }

    // ── Constructor ───────────────────────────────────────────────────

    /// Create a new SSH Tunnel Manager application
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize application state
        let app_state = Arc::new(AppState::new().expect("Failed to initialize application state"));

        // Start session manager's idle monitor in a background task
        let session_manager = app_state.session_manager.clone();
        tokio::spawn(async move {
            session_manager.start_idle_monitor().await;
        });

        // Start background task to refresh sessions (for traffic stats updates)
        let app_state_refresh = app_state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                interval.tick().await;
                let _ = app_state_refresh.reload_sessions().await;
            }
        });

        // Create search input for sidebar
        let search_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder(&t!("search.placeholder").to_string(), window, cx);
            state
        });

        // Create password input
        let password_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder(&t!("connection.enter_password").to_string(), window, cx);
            state.set_masked(true, window, cx);
            state
        });

        // Subscribe to search input changes
        let app_state_clone = app_state.clone();
        cx.subscribe(&search_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.set_filter(text).await;
                });
            }
        })
        .detach();

        // Subscribe to password input changes
        let app_state_clone = app_state.clone();
        cx.subscribe(&password_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.set_password_value(text).await;
                });
            }
        })
        .detach();

        // Create input states for the connection form with placeholders
        let name_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("My SSH Server", window, cx);
            state
        });
        let host_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("example.com", window, cx);
            state
        });
        let port_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("22", window, cx);
            state
        });
        let username_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("root", window, cx);
            state
        });
        let private_key_path_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("~/.ssh/id_rsa", window, cx);
            state
        });
        let local_port_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("8080", window, cx);
            state
        });
        let remote_host_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("localhost", window, cx);
            state
        });
        let remote_port_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("80", window, cx);
            state
        });
        let bind_address_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_placeholder("127.0.0.1", window, cx);
            state
        });

        // Subscribe to input changes for name field
        let app_state_clone = app_state.clone();
        cx.subscribe(&name_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("name", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for host field
        let app_state_clone = app_state.clone();
        cx.subscribe(&host_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("host", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for port field
        let app_state_clone = app_state.clone();
        cx.subscribe(&port_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("port", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for username field
        let app_state_clone = app_state.clone();
        cx.subscribe(&username_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("username", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for private_key_path field
        let app_state_clone = app_state.clone();
        cx.subscribe(
            &private_key_path_input,
            move |_, input, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    let text = input.read(cx).text().to_string();
                    let app_state = app_state_clone.clone();
                    tokio::spawn(async move {
                        app_state.update_form_field("private_key_path", text).await;
                    });
                }
            },
        )
        .detach();

        // Subscribe to input changes for local_port field
        let app_state_clone = app_state.clone();
        cx.subscribe(&local_port_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("local_port", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for remote_host field
        let app_state_clone = app_state.clone();
        cx.subscribe(&remote_host_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("remote_host", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for remote_port field
        let app_state_clone = app_state.clone();
        cx.subscribe(&remote_port_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("remote_port", text).await;
                });
            }
        })
        .detach();

        // Subscribe to input changes for bind_address field
        let app_state_clone = app_state.clone();
        cx.subscribe(&bind_address_input, move |_, input, ev: &InputEvent, cx| {
            if let InputEvent::Change = ev {
                let text = input.read(cx).text().to_string();
                let app_state = app_state_clone.clone();
                tokio::spawn(async move {
                    app_state.update_form_field("bind_address", text).await;
                });
            }
        })
        .detach();

        Self {
            app_state,
            search_input,
            password_input,
            name_input,
            host_input,
            port_input,
            username_input,
            private_key_path_input,
            local_port_input,
            remote_host_input,
            remote_port_input,
            bind_address_input,
            show_advanced: Cell::new(false),
        }
    }

    // ── Card 1: Connection (two-column) ──────────────────────────────

    fn render_connection_card(&self, cx: &mut Context<Self>) -> Div {
        use label::Label;

        let is_dark = cx.theme().mode.is_dark();
        let muted = AppColors::secondary_text(is_dark);

        section_card(&t!("connection.host_info").to_string(), is_dark)
            // Row 1: Name + Host (two-column)
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                Label::new(t!("connection.connection_name").to_string())
                                    .text_size(rems(0.8))
                                    .text_color(muted),
                            )
                            .child(Input::new(&self.name_input).cleanable(true)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                Label::new(t!("connection.host_address").to_string())
                                    .text_size(rems(0.8))
                                    .text_color(muted),
                            )
                            .child(Input::new(&self.host_input).cleanable(true)),
                    ),
            )
            // Row 2: SSH Port + Username (two-column)
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .w(px(120.0))
                            .gap_1()
                            .child(
                                Label::new(t!("connection.ssh_port").to_string())
                                    .text_size(rems(0.8))
                                    .text_color(muted),
                            )
                            .child(Input::new(&self.port_input).cleanable(true)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                Label::new(t!("connection.username").to_string())
                                    .text_size(rems(0.8))
                                    .text_color(muted),
                            )
                            .child(Input::new(&self.username_input).cleanable(true)),
                    ),
            )
    }

    // ── Card 2: Authentication ───────────────────────────────────────

    fn render_authentication_card(&self, cx: &mut Context<Self>) -> Div {
        use label::Label;

        let is_dark = cx.theme().mode.is_dark();
        let border = AppColors::border_color(is_dark);
        let muted = AppColors::secondary_text(is_dark);
        let accent = AppColors::accent_color(is_dark);

        let form_data = if let Ok(ui_state) = self.app_state.ui_state.try_read() {
            ui_state.form_data.clone()
        } else {
            ConnectionFormData::default()
        };

        let is_publickey = form_data.auth_type == "publickey";

        section_card(&t!("connection.authentication").to_string(), is_dark)
            // Segmented control
            .child(
                h_flex()
                    .gap_0()
                    .child({
                        let app_state = self.app_state.clone();
                        div()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_l_lg()
                            .border_1()
                            .border_color(if !is_publickey { accent } else { border })
                            .bg(if !is_publickey {
                                accent.opacity(0.10)
                            } else {
                                gpui::transparent_black()
                            })
                            .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
                                let app_state = app_state.clone();
                                tokio::spawn(async move {
                                    app_state
                                        .update_form_field("auth_type", "password".to_string())
                                        .await;
                                });
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(if !is_publickey {
                                        FontWeight::MEDIUM
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if !is_publickey { accent } else { muted })
                                    .child(t!("connection.password").to_string()),
                            )
                    })
                    .child({
                        let app_state = self.app_state.clone();
                        div()
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_r_lg()
                            .border_1()
                            .border_color(if is_publickey { accent } else { border })
                            .bg(if is_publickey {
                                accent.opacity(0.10)
                            } else {
                                gpui::transparent_black()
                            })
                            .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
                                let app_state = app_state.clone();
                                tokio::spawn(async move {
                                    app_state
                                        .update_form_field("auth_type", "publickey".to_string())
                                        .await;
                                });
                            })
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(if is_publickey {
                                        FontWeight::MEDIUM
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if is_publickey { accent } else { muted })
                                    .child(t!("connection.public_key").to_string()),
                            )
                    }),
            )
            // Conditional content
            .child(if is_publickey {
                v_flex()
                    .gap_2()
                    .child(
                        Label::new(t!("connection.private_key_path").to_string())
                            .text_size(rems(0.8))
                            .text_color(muted),
                    )
                    .child(Input::new(&self.private_key_path_input).cleanable(true))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Passphrase will be requested on connect if required"),
                    )
            } else {
                v_flex().gap_1().child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child(t!("connection.password_hint").to_string()),
                )
            })
    }

    // ── Card 4: Advanced (collapsible) ───────────────────────────────

    fn render_advanced_card(&self, cx: &mut Context<Self>) -> Div {
        use label::Label;

        let is_dark = cx.theme().mode.is_dark();
        let card_bg = AppColors::card_bg(is_dark);
        let border = AppColors::border_color(is_dark);
        let text = AppColors::primary_text(is_dark);
        let muted = AppColors::secondary_text(is_dark);

        let expanded = self.show_advanced.get();

        let (compression, quiet_mode) = if let Ok(ui_state) = self.app_state.ui_state.try_read() {
            (
                ui_state.form_data.compression,
                ui_state.form_data.quiet_mode,
            )
        } else {
            (true, false)
        };

        v_flex()
            .p_4()
            .bg(card_bg)
            .border_1()
            .border_color(border)
            .rounded_lg()
            // Title bar (always visible, clickable to toggle)
            .child(
                div()
                    .id("advanced_toggle")
                    .cursor_pointer()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                Label::new(t!("connection.advanced_options").to_string())
                                    .text_size(rems(0.95))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text),
                            )
                            .child(div().text_sm().text_color(muted).child(if expanded {
                                "v"
                            } else {
                                ">"
                            })),
                    )
                    .on_mouse_down(gpui::MouseButton::Left, {
                        let show_advanced = &self.show_advanced as *const Cell<bool>;
                        move |_event, window, _app| {
                            // SAFETY: show_advanced lives as long as SshTunnelApp
                            let cell = unsafe { &*show_advanced };
                            cell.set(!cell.get());
                            window.refresh();
                        }
                    }),
            )
            // Summary when collapsed
            .when(!expanded, |this| {
                this.child(
                    div()
                        .mt_1()
                        .text_xs()
                        .text_color(muted)
                        .child("Compression, host key verification, timeout"),
                )
            })
            // Content (only when expanded)
            .when(expanded, |this| {
                let app_state_compression = self.app_state.clone();
                let app_state_quiet = self.app_state.clone();

                this.child(
                    h_flex()
                        .mt_3()
                        .gap_4()
                        .child(Self::render_checkbox(
                            "compression_toggle",
                            t!("connection.compression").to_string(),
                            compression,
                            card_bg,
                            border,
                            text,
                            muted,
                            if is_dark {
                                hsla(0.0, 0.0, 0.18, 1.0)
                            } else {
                                hsla(0.0, 0.0, 0.96, 1.0)
                            },
                            move || {
                                let app_state = app_state_compression.clone();
                                tokio::spawn(async move {
                                    app_state.toggle_compression().await;
                                });
                            },
                        ))
                        .child(Self::render_checkbox(
                            "quiet_mode_toggle",
                            t!("connection.quiet_mode").to_string(),
                            quiet_mode,
                            card_bg,
                            border,
                            text,
                            muted,
                            if is_dark {
                                hsla(0.0, 0.0, 0.18, 1.0)
                            } else {
                                hsla(0.0, 0.0, 0.96, 1.0)
                            },
                            move || {
                                let app_state = app_state_quiet.clone();
                                tokio::spawn(async move {
                                    app_state.toggle_quiet_mode().await;
                                });
                            },
                        )),
                )
            })
    }

    /// Render a checkbox toggle with label
    fn render_checkbox<F>(
        id: &'static str,
        label_text: String,
        checked: bool,
        card_bg: Hsla,
        border_color: Hsla,
        text_color: Hsla,
        muted_color: Hsla,
        muted_bg: Hsla,
        on_toggle: F,
    ) -> Stateful<Div>
    where
        F: Fn() + 'static,
    {
        use label::Label;
        let success_color = AppColors::success_color();

        div()
            .id(id)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .bg(if checked {
                        success_color.opacity(0.1)
                    } else {
                        muted_bg
                    })
                    .rounded_lg()
                    .child(
                        div()
                            .size(px(16.0))
                            .rounded_sm()
                            .border_1()
                            .border_color(if checked { success_color } else { border_color })
                            .bg(if checked { success_color } else { card_bg })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(checked, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                        .child("✓"),
                                )
                            }),
                    )
                    .child(
                        Label::new(label_text)
                            .text_size(rems(0.85))
                            .text_color(if checked { text_color } else { muted_color }),
                    ),
            )
            .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, _app| {
                on_toggle();
            })
    }

    // ── Template Selector ────────────────────────────────────────────

    fn render_template_selector(&self, cx: &mut Context<Self>) -> Div {
        use label::Label;

        let is_dark = cx.theme().mode.is_dark();
        let card_bg = AppColors::card_bg(is_dark);
        let border = AppColors::border_color(is_dark);
        let text = AppColors::primary_text(is_dark);
        let muted = AppColors::secondary_text(is_dark);
        let page_bg = AppColors::page_bg(is_dark);

        let templates = vec![
            (
                "mysql",
                t!("template.mysql_name").to_string(),
                t!("template.mysql_desc").to_string(),
            ),
            (
                "postgresql",
                t!("template.postgresql_name").to_string(),
                t!("template.postgresql_desc").to_string(),
            ),
            (
                "web",
                t!("template.web_name").to_string(),
                t!("template.web_desc").to_string(),
            ),
            (
                "socks5",
                t!("template.socks5_name").to_string(),
                t!("template.socks5_desc").to_string(),
            ),
            (
                "rdp",
                t!("template.rdp_name").to_string(),
                t!("template.rdp_desc").to_string(),
            ),
            (
                "remote",
                t!("template.remote_name").to_string(),
                t!("template.remote_desc").to_string(),
            ),
        ];

        v_flex()
            .flex_shrink_0()
            .p_4()
            .bg(page_bg)
            .border_b_1()
            .border_color(border)
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .mb_3()
                    .child(
                        Label::new(t!("app.quick_templates").to_string())
                            .text_size(rems(0.95))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text),
                    )
                    .child({
                        let app_state = self.app_state.clone();
                        div()
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_lg()
                            .text_xs()
                            .text_color(muted)
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                                let app_state = app_state.clone();
                                tokio::spawn(async move {
                                    app_state.toggle_templates().await;
                                });
                            })
                            .child(t!("app.close").to_string())
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(templates.into_iter().map(|(id, name, desc)| {
                        let app_state = self.app_state.clone();
                        let template_id = id.to_string();

                        div()
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .bg(card_bg)
                            .border_1()
                            .border_color(border)
                            .rounded_lg()
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                                let app_state = app_state.clone();
                                let template_id = template_id.clone();
                                tokio::spawn(async move {
                                    app_state.load_template(&template_id).await;
                                    app_state.toggle_templates().await;
                                });
                            })
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text)
                                            .child(name),
                                    )
                                    .child(div().text_xs().text_color(muted).child(desc)),
                            )
                    })),
            )
    }

    // ── Helpers ───────────────────────────────────────────────────────

    /// Helper: Format bytes to human-readable string
    fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Helper: Format duration to human-readable string
    fn format_duration(duration: chrono::Duration) -> String {
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        let seconds = duration.num_seconds() % 60;

        if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    // ── Notifications ────────────────────────────────────────────────

    fn render_notifications(&self, cx: &mut Context<Self>) -> Option<Div> {
        let ui_state = if let Ok(state) = self.app_state.ui_state.try_read() {
            state.clone()
        } else {
            return None;
        };

        let app_state = self.app_state.clone();
        let is_dark = cx.theme().mode.is_dark();

        // Notification colors
        let error_bg = if is_dark {
            hsla(0.0, 0.40, 0.20, 1.0)
        } else {
            hsla(0.0, 0.86, 0.94, 1.0)
        };
        let error_border = AppColors::danger_color();
        let error_text = if is_dark {
            hsla(0.0, 0.75, 0.80, 1.0)
        } else {
            hsla(0.0, 0.70, 0.35, 1.0)
        };

        let warning_bg = if is_dark {
            hsla(38.0 / 360.0, 0.40, 0.20, 1.0)
        } else {
            hsla(45.0 / 360.0, 0.93, 0.89, 1.0)
        };
        let warning_border = hsla(38.0 / 360.0, 0.92, 0.50, 1.0);
        let warning_text = if is_dark {
            hsla(38.0 / 360.0, 0.80, 0.70, 1.0)
        } else {
            hsla(28.0 / 360.0, 0.80, 0.31, 1.0)
        };

        let info_bg = if is_dark {
            hsla(217.0 / 360.0, 0.40, 0.20, 1.0)
        } else {
            hsla(214.0 / 360.0, 0.95, 0.93, 1.0)
        };
        let info_border = hsla(217.0 / 360.0, 0.91, 0.60, 1.0);
        let info_text = if is_dark {
            hsla(217.0 / 360.0, 0.80, 0.75, 1.0)
        } else {
            hsla(224.0 / 360.0, 0.76, 0.40, 1.0)
        };

        let success_bg = if is_dark {
            hsla(152.0 / 360.0, 0.40, 0.15, 1.0)
        } else {
            hsla(149.0 / 360.0, 0.80, 0.90, 1.0)
        };
        let success_border = hsla(160.0 / 360.0, 0.84, 0.39, 1.0);
        let success_text = if is_dark {
            hsla(152.0 / 360.0, 0.70, 0.70, 1.0)
        } else {
            hsla(160.0 / 360.0, 0.84, 0.20, 1.0)
        };

        if let Some(error) = &ui_state.error_message {
            let (bg_color, bdr_color, txt_color, icon) = match error.severity {
                ErrorSeverity::Error => (error_bg, error_border, error_text, "X"),
                ErrorSeverity::Warning => (warning_bg, warning_border, warning_text, "!"),
                ErrorSeverity::Info => (info_bg, info_border, info_text, "i"),
            };

            Some(
                v_flex()
                    .p_3()
                    .mb_2()
                    .bg(bg_color)
                    .border_1()
                    .border_color(bdr_color)
                    .rounded_lg()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(txt_color)
                                            .child(icon),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(txt_color)
                                            .child(error.message.clone()),
                                    ),
                            )
                            .child({
                                use button::Button;
                                Button::new("close_error").label("×".to_string()).on_click(
                                    move |_, _, _| {
                                        let app_state = app_state.clone();
                                        tokio::spawn(async move {
                                            app_state.clear_notifications().await;
                                        });
                                    },
                                )
                            }),
                    ),
            )
        } else if let Some(success) = &ui_state.success_message {
            Some(
                v_flex()
                    .p_3()
                    .mb_2()
                    .bg(success_bg)
                    .border_1()
                    .border_color(success_border)
                    .rounded_lg()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(success_text)
                                            .child("OK"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(success_text)
                                            .child(success.clone()),
                                    ),
                            )
                            .child({
                                use button::Button;
                                Button::new("close_success")
                                    .label("×".to_string())
                                    .on_click(move |_, _, _| {
                                        let app_state = app_state.clone();
                                        tokio::spawn(async move {
                                            app_state.clear_notifications().await;
                                        });
                                    })
                            }),
                    ),
            )
        } else {
            None
        }
    }

    // ── Sessions Panel ───────────────────────────────────────────────

    fn render_sessions_panel(&self, cx: &mut Context<Self>) -> Div {
        use button::Button;
        use label::Label;

        let is_dark = cx.theme().mode.is_dark();
        let border = AppColors::border_color(is_dark);
        let card_bg = AppColors::card_bg(is_dark);
        let text = AppColors::primary_text(is_dark);
        let muted = AppColors::secondary_text(is_dark);
        let success = AppColors::success_color();

        let session_bg = if is_dark {
            hsla(142.0 / 360.0, 0.30, 0.12, 1.0)
        } else {
            hsla(142.0 / 360.0, 0.76, 0.97, 1.0)
        };
        let session_border = if is_dark {
            hsla(142.0 / 360.0, 0.50, 0.25, 1.0)
        } else {
            hsla(149.0 / 360.0, 0.80, 0.90, 1.0)
        };
        let session_title_color = if is_dark {
            hsla(142.0 / 360.0, 0.70, 0.70, 1.0)
        } else {
            hsla(144.0 / 360.0, 0.75, 0.20, 1.0)
        };

        let sessions = if let Ok(sess) = self.app_state.sessions.try_read() {
            sess.clone()
        } else {
            vec![]
        };

        let session_count = sessions.len();

        if sessions.is_empty() {
            return div();
        }

        v_flex()
            .flex_shrink_0()
            .max_h(px(200.0))
            .overflow_hidden()
            .border_t_1()
            .border_color(border)
            .bg(session_bg)
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size(px(8.0))
                                            .rounded_full()
                                            .bg(success),
                                    )
                                    .child(
                                        Label::new(format!(
                                            "{} ({})",
                                            t!("app.active_sessions"),
                                            session_count
                                        ))
                                        .text_size(rems(0.9))
                                        .text_color(session_title_color),
                                    ),
                            ),
                    )
                    .children(sessions.into_iter().enumerate().map(|(idx, session)| {
                        let session_id = session.id;
                        let app_state = self.app_state.clone();
                        let duration =
                            chrono::Utc::now().signed_duration_since(session.started_at);
                        let duration_str = Self::format_duration(duration);
                        // Memory intentionally leaked for GPUI element ID stability across renders
                        let btn_id: &'static str =
                            Box::leak(format!("disconnect_{}", idx).into_boxed_str());

                        h_flex()
                            .px_3()
                            .py_2()
                            .bg(card_bg)
                            .rounded_lg()
                            .border_1()
                            .border_color(session_border)
                            .items_center()
                            .justify_between()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        Label::new(session.connection_name.clone())
                                            .text_size(rems(0.85))
                                            .text_color(text),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_3()
                                            .child(
                                                div().text_xs().text_color(muted).child(
                                                    t!("session.duration", "duration" => duration_str.as_str())
                                                        .to_string(),
                                                ),
                                            )
                                            .child(
                                                div().text_xs().text_color(muted).child(
                                                    t!("session.traffic",
                                                        sent = Self::format_bytes(session.bytes_sent),
                                                        received = Self::format_bytes(session.bytes_received)
                                                    )
                                                    .to_string(),
                                                ),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new(btn_id)
                                    .danger()
                                    .compact()
                                    .label(t!("actions.disconnect").to_string())
                                    .on_click(move |_, _, _| {
                                        let app_state = app_state.clone();
                                        tokio::spawn(async move {
                                            if let Err(e) =
                                                app_state.disconnect_session(session_id).await
                                            {
                                                tracing::error!(
                                                    "Failed to disconnect: {}",
                                                    e
                                                );
                                            } else {
                                                tracing::info!(
                                                    "Session {} disconnected",
                                                    session_id
                                                );
                                            }
                                        });
                                    }),
                            )
                    })),
            )
    }

    // ── Right Panel — Detail Workbench ───────────────────────────────

    fn render_right_panel(&self, cx: &mut Context<Self>) -> Div {
        let is_dark = cx.theme().mode.is_dark();
        let page_bg = AppColors::page_bg(is_dark);

        // Get UI state
        let (form_data, editing_id, password_input_for, is_connecting, show_templates) =
            if let Ok(ui_state) = self.app_state.ui_state.try_read() {
                (
                    ui_state.form_data.clone(),
                    ui_state.editing_connection_id,
                    ui_state.password_input_for,
                    !ui_state.connecting_ids.is_empty(),
                    ui_state.show_templates,
                )
            } else {
                (ConnectionFormData::default(), None, None, false, false)
            };

        // Get active session for this connection
        let is_this_connected = if let Some(eid) = editing_id {
            if let Ok(sessions) = self.app_state.sessions.try_read() {
                sessions.iter().any(|s| s.connection_id == eid)
            } else {
                false
            }
        } else {
            false
        };

        let is_editing = editing_id.is_some();
        let needs_password = password_input_for.is_some();

        v_flex()
            .flex_1()
            .size_full()
            .overflow_hidden()
            .bg(page_bg)
            // ── Fixed header (delegated to component) ──
            .child(render_connection_header(
                &self.app_state,
                &form_data,
                editing_id,
                is_connecting,
                is_this_connected,
                is_dark,
            ))
            // Password input section (shown when needed)
            .when(needs_password, |this| {
                this.child(render_password_section(
                    &self.app_state,
                    &self.password_input,
                    password_input_for.unwrap(),
                    is_dark,
                ))
            })
            // Template selector panel
            .when(show_templates && !is_editing, |this| {
                this.child(self.render_template_selector(cx))
            })
            // ── Scrollable form area with 4 cards ──
            .child(
                div().flex_1().min_h_0().overflow_y_scrollbar().child(
                    v_flex()
                        .gap_4()
                        .w_full()
                        .p_4()
                        .pb_8()
                        .child(self.render_connection_card(cx))
                        .child(self.render_authentication_card(cx))
                        .child(render_tunnel_card(
                            &self.app_state,
                            &form_data,
                            &self.local_port_input,
                            &self.remote_host_input,
                            &self.remote_port_input,
                            &self.bind_address_input,
                            is_dark,
                        ))
                        .child(self.render_advanced_card(cx)),
                ),
            )
            // Active sessions panel
            .child(self.render_sessions_panel(cx))
    }
}

impl Render for SshTunnelApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Sync form_data to inputs on every render
        self.sync_form_to_inputs(window, cx);

        let is_dark = cx.theme().mode.is_dark();
        let page_bg = AppColors::page_bg(is_dark);
        let border = AppColors::border_color(is_dark);

        v_flex()
            .size_full()
            .overflow_hidden()
            .bg(page_bg)
            // Header (fixed, delegated to component)
            .child(
                div()
                    .flex_shrink_0()
                    .child(render_top_bar(&self.app_state, is_dark)),
            )
            // Notifications
            .when_some(self.render_notifications(cx), |this, notification| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .px_4()
                        .pt_3()
                        .pb_3()
                        .border_b_1()
                        .border_color(border)
                        .child(notification),
                )
            })
            // Main split layout
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(render_connection_sidebar(
                        &self.app_state,
                        &self.search_input,
                        is_dark,
                    ))
                    .child(self.render_right_panel(cx)),
            )
    }
}
