// Load i18n translations
rust_i18n::i18n!("locales", fallback = "en");

// Re-export from library
use ssh_tunnel_manager::{models, services, utils};

mod cli;

// GUI modules (requires Rust 1.87+ and Xcode Command Line Tools)
#[cfg(feature = "gui")]
mod ui;

use clap::Parser;
use cli::{Cli, run_interactive};
use utils::{i18n, logger};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger
    logger::init();

    // Set default language
    i18n::set_language();

    tracing::info!("SSH Tunnel Manager - Starting...");
    tracing::info!("Current language: {}", i18n::current_language());

    let cli = Cli::parse();

    // Helper: find a connection by name (or UUID prefix)
    let find_connection =
        |name: &str, connections: &[models::SshConnection]| -> Option<models::SshConnection> {
            // Try exact name match first
            if let Some(conn) = connections.iter().find(|c| c.name == name) {
                return Some(conn.clone());
            }
            // Try UUID prefix match
            if let Ok(id) = uuid::Uuid::parse_str(name) {
                return connections.iter().find(|c| c.id == id).cloned();
            }
            None
        };

    if cli.gui {
        #[cfg(feature = "gui")]
        {
            // Run GUI mode (requires Rust 1.87+ and Xcode Command Line Tools)
            tracing::info!("Starting GUI mode...");
            ui::run_gui()?;
        }
        #[cfg(not(feature = "gui"))]
        {
            eprintln!("GUI feature is not enabled. Please compile with --features gui");
            eprintln!("Note: GUI requires Xcode Command Line Tools on macOS.");
            eprintln!();
            eprintln!("To install Xcode Command Line Tools:");
            eprintln!("  xcode-select --install");
            eprintln!();
            eprintln!("Then rebuild with GUI support:");
            eprintln!("  cargo build --release --features gui");
            std::process::exit(1);
        }
    } else if cli.interactive || cli.command.is_none() {
        // Run interactive CLI mode
        run_interactive().await?;
    } else {
        // Handle command-line commands
        match cli.command {
            Some(cli::commands::Commands::List) => {
                let config = services::config_service::ConfigService::new()?;
                let connections = config.load_connections()?;

                if connections.is_empty() {
                    println!("{}", rust_i18n::t!("connection.no_connections"));
                } else {
                    for conn in connections {
                        println!(
                            "{} - {}@{}:{}",
                            conn.name, conn.username, conn.host, conn.port
                        );
                    }
                }
            }
            Some(cli::commands::Commands::Add {
                name,
                host,
                port,
                username,
                password,
                key,
            }) => {
                let config = services::config_service::ConfigService::new()?;
                let auth_method = if let Some(key_path) = key {
                    models::AuthMethod::PublicKey {
                        private_key_path: std::path::PathBuf::from(key_path),
                        passphrase_required: false,
                    }
                } else if password {
                    models::AuthMethod::Password
                } else {
                    models::AuthMethod::Password
                };

                let connection = models::SshConnection {
                    id: uuid::Uuid::new_v4(),
                    name: name.clone(),
                    host,
                    port,
                    username,
                    auth_method,
                    forwarding_configs: Vec::new(),
                    jump_hosts: Vec::new(),
                    idle_timeout_seconds: Some(300),
                    host_key_fingerprint: None,
                    verify_host_key: false,
                    compression: true,
                    quiet_mode: false,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                config.save_connection(&connection)?;
                println!("Connection '{}' added ({})", name, connection.id);
            }
            Some(cli::commands::Commands::Connect { name, password }) => {
                let config = services::config_service::ConfigService::new()?;
                let connections = config.load_connections()?;

                let connection = find_connection(&name, &connections).ok_or_else(|| {
                    anyhow::anyhow!("Connection '{}' not found", name)
                })?;

                let session_manager = std::sync::Arc::new(
                    services::session_manager::SessionManager::new(
                        connection.idle_timeout_seconds.unwrap_or(300),
                    ),
                );
                session_manager.start_idle_monitor().await;

                let ssh_session = services::ssh_service::SshService::connect(
                    &connection,
                    password.as_deref(),
                )
                .await?;

                let session_id =
                    session_manager.create_session(connection.clone(), ssh_session).await?;

                // Set up tunnels if any forwarding is configured
                if !connection.forwarding_configs.is_empty() {
                    session_manager
                        .setup_tunnels(session_id, &connection.forwarding_configs)
                        .await?;
                    for fwd in &connection.forwarding_configs {
                        println!("  Tunnel: {}", fwd.description());
                    }
                }

                // Write session file so other CLI invocations can see it
                let session_store =
                    services::session_file::SessionFileStore::new(config.config_dir())?;
                let session_record = services::session_file::SessionRecord {
                    session_id,
                    connection_name: connection.name.clone(),
                    host: connection.host.clone(),
                    port: connection.port,
                    username: connection.username.clone(),
                    pid: std::process::id(),
                    started_at: chrono::Utc::now(),
                    forwarding_descriptions: connection
                        .forwarding_configs
                        .iter()
                        .map(|f| f.description())
                        .collect(),
                };
                session_store.write(&session_record)?;

                println!("Connected to '{}' (session: {})", connection.name, session_id);
                println!("Press Ctrl+C to disconnect...");

                // Wait for either SIGINT (Ctrl+C) or SIGTERM (from `disconnect` command)
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{SignalKind, signal};
                    let mut sigterm = signal(SignalKind::terminate())?;
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {}
                        _ = sigterm.recv() => {}
                    }
                }
                #[cfg(not(unix))]
                {
                    tokio::signal::ctrl_c().await?;
                }

                session_manager.disconnect_session(session_id).await?;
                session_store.remove(session_id)?;
                println!("Disconnected.");
            }
            Some(cli::commands::Commands::Delete { name }) => {
                let config = services::config_service::ConfigService::new()?;
                let connections = config.load_connections()?;

                let connection = find_connection(&name, &connections).ok_or_else(|| {
                    anyhow::anyhow!("Connection '{}' not found", name)
                })?;

                if config.delete_connection(connection.id)? {
                    println!("Connection '{}' deleted", connection.name);
                } else {
                    println!("Connection '{}' not found", name);
                }
            }
            Some(cli::commands::Commands::Show { name }) => {
                let config = services::config_service::ConfigService::new()?;
                let connections = config.load_connections()?;

                let conn = find_connection(&name, &connections).ok_or_else(|| {
                    anyhow::anyhow!("Connection '{}' not found", name)
                })?;

                println!("Name:     {}", conn.name);
                println!("ID:       {}", conn.id);
                println!("Host:     {}:{}", conn.host, conn.port);
                println!("Username: {}", conn.username);
                println!(
                    "Auth:     {}",
                    match &conn.auth_method {
                        models::AuthMethod::Password => "Password".to_string(),
                        models::AuthMethod::PublicKey {
                            private_key_path, ..
                        } => format!("PublicKey ({})", private_key_path.display()),
                    }
                );
                if !conn.forwarding_configs.is_empty() {
                    println!("Forwarding:");
                    for fwd in &conn.forwarding_configs {
                        println!("  - {}", fwd.description());
                    }
                }
                if !conn.jump_hosts.is_empty() {
                    println!("Jump Hosts:");
                    for jh in &conn.jump_hosts {
                        println!("  - {}@{}:{}", jh.username, jh.host, jh.port);
                    }
                }
                println!("Created:  {}", conn.created_at.format("%Y-%m-%d %H:%M:%S"));
            }
            Some(cli::commands::Commands::Sessions) => {
                let config = services::config_service::ConfigService::new()?;
                let store =
                    services::session_file::SessionFileStore::new(config.config_dir())?;
                let sessions = store.list_active()?;

                if sessions.is_empty() {
                    println!("No active sessions.");
                } else {
                    for s in sessions {
                        let elapsed = chrono::Utc::now() - s.started_at;
                        let mins = elapsed.num_minutes();
                        let secs = elapsed.num_seconds() % 60;
                        println!(
                            "{} - {} ({}@{}:{}) pid:{} uptime:{}m{}s",
                            s.session_id,
                            s.connection_name,
                            s.username,
                            s.host,
                            s.port,
                            s.pid,
                            mins,
                            secs
                        );
                        for desc in &s.forwarding_descriptions {
                            println!("  {}", desc);
                        }
                    }
                }
            }
            Some(cli::commands::Commands::Disconnect { id }) => {
                let session_id = uuid::Uuid::parse_str(&id)
                    .map_err(|_| anyhow::anyhow!("Invalid session ID: {}", id))?;

                let config = services::config_service::ConfigService::new()?;
                let store =
                    services::session_file::SessionFileStore::new(config.config_dir())?;

                match store.find(session_id)? {
                    Some(record) => {
                        // Send SIGTERM to the connect process
                        #[cfg(unix)]
                        {
                            let ret = unsafe { libc::kill(record.pid as i32, libc::SIGTERM) };
                            if ret == 0 {
                                println!(
                                    "Sent disconnect signal to session {} (pid {})",
                                    session_id, record.pid
                                );
                                // The connect process will clean up its own session file on exit
                            } else {
                                // Process already gone, clean up stale file
                                store.remove(session_id)?;
                                println!("Session {} is no longer running (cleaned up)", session_id);
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = record;
                            println!("Disconnect via signal is only supported on Unix systems");
                        }
                    }
                    None => {
                        println!("Session {} not found or already terminated", session_id);
                    }
                }
            }
            Some(cli::commands::Commands::Forward {
                connection,
                r#type,
                local_port,
                remote_host,
                remote_port,
            }) => {
                let config = services::config_service::ConfigService::new()?;
                let connections = config.load_connections()?;

                let mut conn = find_connection(&connection, &connections).ok_or_else(|| {
                    anyhow::anyhow!("Connection '{}' not found", connection)
                })?;

                let fwd = match r#type.as_str() {
                    "local" => {
                        let rh = remote_host.unwrap_or_else(|| "localhost".to_string());
                        let rp = remote_port.ok_or_else(|| {
                            anyhow::anyhow!("--remote-port is required for local forwarding")
                        })?;
                        models::ForwardingConfig::local(local_port, rh, rp)
                    }
                    "remote" => {
                        let rp = remote_port.ok_or_else(|| {
                            anyhow::anyhow!("--remote-port is required for remote forwarding")
                        })?;
                        models::ForwardingConfig::remote(rp, "localhost", local_port)
                    }
                    "dynamic" => models::ForwardingConfig::dynamic(local_port),
                    other => {
                        anyhow::bail!(
                            "Unknown forwarding type '{}'. Use: local, remote, dynamic",
                            other
                        );
                    }
                };

                println!("Added forwarding: {}", fwd.description());
                conn.forwarding_configs.push(fwd);
                conn.updated_at = chrono::Utc::now();
                config.save_connection(&conn)?;
            }
            Some(cli::commands::Commands::Templates) => {
                let templates = models::ConnectionTemplate::builtin_templates();
                for template in templates {
                    println!("{} - {}", template.name, template.description);
                    for fwd in &template.forwarding_presets {
                        println!("  {}", fwd.description());
                    }
                }
            }
            Some(cli::commands::Commands::FromTemplate {
                template,
                name,
                host,
                username,
            }) => {
                let templates = models::ConnectionTemplate::builtin_templates();
                let tmpl = templates
                    .iter()
                    .find(|t| t.name.eq_ignore_ascii_case(&template))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Template '{}' not found. Available: {}",
                            template,
                            templates
                                .iter()
                                .map(|t| t.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })?;

                let connection = models::SshConnection {
                    id: uuid::Uuid::new_v4(),
                    name: name.clone(),
                    host,
                    port: tmpl.default_port,
                    username,
                    auth_method: tmpl.default_auth_method.clone(),
                    forwarding_configs: tmpl.forwarding_presets.clone(),
                    jump_hosts: Vec::new(),
                    idle_timeout_seconds: Some(300),
                    host_key_fingerprint: None,
                    verify_host_key: false,
                    compression: true,
                    quiet_mode: false,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                let config = services::config_service::ConfigService::new()?;
                config.save_connection(&connection)?;
                println!(
                    "Connection '{}' created from template '{}'",
                    name, tmpl.name
                );
                for fwd in &connection.forwarding_configs {
                    println!("  {}", fwd.description());
                }
            }
            None => unreachable!(),
        }
    }

    Ok(())
}
