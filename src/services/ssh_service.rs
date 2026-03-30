use crate::models::forwarding::RemoteForwarding;
use crate::models::{AuthMethod, JumpHost, SshConnection};
use crate::utils::error::{Result, SshToolError};
use russh::client::{self, AuthResult, Handle, Msg}; // client types
use russh::{Channel, ChannelMsg, Disconnect};
// Note: In russh 0.55.0, key types are re-exported in russh::keys
use russh::keys::{PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::RwLock;

/// SSH client session handle
pub type SshSession = Handle<SshClientHandler>;

/// Shared remote forwards configuration (used across session and tunnels)
#[allow(dead_code)]
pub type SharedRemoteForwards = Arc<RwLock<Vec<RemoteForwarding>>>;

/// Create a `DuplexStream` that bridges a russh `Channel<Msg>` for use
/// with `client::connect_stream`. A background task handles bidirectional
/// copying between the channel and one end of the duplex; the other end
/// is returned to the caller.
fn channel_to_stream(mut channel: Channel<Msg>) -> tokio::io::DuplexStream {
    let (caller_side, mut bridge_side) = tokio::io::duplex(64 * 1024);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        loop {
            tokio::select! {
                // bridge_side read → channel write
                result = bridge_side.read(&mut buf) => {
                    match result {
                        Ok(0) | Err(_) => {
                            let _ = channel.eof().await;
                            break;
                        }
                        Ok(n) => {
                            if channel.data(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                // channel read → bridge_side write
                msg = channel.wait() => {
                    match msg {
                        Some(ChannelMsg::Data { ref data }) => {
                            if bridge_side.write_all(data).await.is_err() {
                                break;
                            }
                        }
                        Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break,
                        Some(_) => {}
                    }
                }
            }
        }
    });

    caller_side
}

/// SSH service for managing connections
pub struct SshService;

impl SshService {
    /// Connect to SSH server with password authentication
    pub async fn connect_password(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        host_key_fingerprint: Option<String>,
        verify_host_key: bool,
        remote_forwards: Vec<RemoteForwarding>,
    ) -> Result<SshSession> {
        tracing::info!(
            "Connecting to {}:{} as {} (password auth)",
            host,
            port,
            username
        );

        let config = client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(300)),
            ..<client::Config as Default>::default()
        };

        let sh = if !remote_forwards.is_empty() {
            tracing::info!(
                "Creating handler with {} remote forward(s)",
                remote_forwards.len()
            );
            let handler = if verify_host_key {
                SshClientHandler::with_verification(host_key_fingerprint)
            } else {
                SshClientHandler::new()
            };
            // Add remote forwards to handler
            for forward in remote_forwards {
                handler.add_remote_forward(forward).await;
            }
            handler
        } else if verify_host_key {
            SshClientHandler::with_verification(host_key_fingerprint)
        } else {
            SshClientHandler::new()
        };

        let mut session = client::connect(Arc::new(config), (host, port), sh)
            .await
            .map_err(|e| SshToolError::SshConnectionFailed(e.to_string()))?;

        let auth_res = session
            .authenticate_password(username, password)
            .await
            .map_err(|e| SshToolError::AuthenticationFailed(e.to_string()))?;

        // In russh 0.55.0, AuthResult is an enum, not a bool
        if !matches!(auth_res, AuthResult::Success) {
            return Err(SshToolError::AuthenticationFailed(
                "Password authentication failed".to_string(),
            ));
        }

        tracing::info!("Successfully authenticated with password");
        Ok(session)
    }

    /// Connect to SSH server with public key authentication
    #[allow(clippy::too_many_arguments)]
    pub async fn connect_pubkey(
        host: &str,
        port: u16,
        username: &str,
        key_path: &Path,
        passphrase: Option<&str>,
        host_key_fingerprint: Option<String>,
        verify_host_key: bool,
        remote_forwards: Vec<RemoteForwarding>,
    ) -> Result<SshSession> {
        tracing::info!(
            "Connecting to {}:{} as {} (pubkey auth)",
            host,
            port,
            username
        );

        // Load private key (expand ~ in path)
        let expanded_key_path = crate::utils::path::expand_tilde(key_path);
        let key_data = tokio::fs::read_to_string(&expanded_key_path)
            .await
            .map_err(|_| SshToolError::KeyFileNotFound(expanded_key_path.display().to_string()))?;

        // In russh 0.55.0, use ssh_key::PrivateKey::from_openssh
        let key = if let Some(pass) = passphrase {
            PrivateKey::from_openssh(key_data.trim())
                .map_err(|e| {
                    SshToolError::AuthenticationFailed(format!("Failed to load key: {}", e))
                })?
                .decrypt(pass.as_bytes())
                .map_err(|e| {
                    SshToolError::AuthenticationFailed(format!("Failed to decrypt key: {}", e))
                })?
        } else {
            PrivateKey::from_openssh(key_data.trim()).map_err(|e| {
                SshToolError::AuthenticationFailed(format!("Failed to load key: {}", e))
            })?
        };

        let config = client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(300)),
            ..<client::Config as Default>::default()
        };

        let sh = if !remote_forwards.is_empty() {
            tracing::info!(
                "Creating handler with {} remote forward(s)",
                remote_forwards.len()
            );
            let handler = if verify_host_key {
                SshClientHandler::with_verification(host_key_fingerprint)
            } else {
                SshClientHandler::new()
            };
            // Add remote forwards to handler
            for forward in remote_forwards {
                handler.add_remote_forward(forward).await;
            }
            handler
        } else if verify_host_key {
            SshClientHandler::with_verification(host_key_fingerprint)
        } else {
            SshClientHandler::new()
        };

        let mut session = client::connect(Arc::new(config), (host, port), sh)
            .await
            .map_err(|e| SshToolError::SshConnectionFailed(e.to_string()))?;

        // In russh 0.55.0, authenticate_publickey expects PrivateKeyWithHashAlg
        let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);
        let auth_res = session
            .authenticate_publickey(username, key_with_alg)
            .await
            .map_err(|e| SshToolError::AuthenticationFailed(e.to_string()))?;

        // In russh 0.55.0, AuthResult is an enum, not a bool
        if !matches!(auth_res, AuthResult::Success) {
            return Err(SshToolError::AuthenticationFailed(
                "Public key authentication failed".to_string(),
            ));
        }

        tracing::info!("Successfully authenticated with public key");
        Ok(session)
    }

    /// Authenticate an SSH session over an arbitrary async stream.
    async fn connect_over_stream<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
        stream: S,
        username: &str,
        auth_method: &AuthMethod,
        password: Option<&str>,
        host_key_fingerprint: Option<String>,
        verify_host_key: bool,
        remote_forwards: Vec<RemoteForwarding>,
    ) -> Result<SshSession> {
        let config = client::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(300)),
            ..<client::Config as Default>::default()
        };

        let handler = if verify_host_key {
            SshClientHandler::with_verification(host_key_fingerprint)
        } else {
            SshClientHandler::new()
        };

        for forward in &remote_forwards {
            handler.add_remote_forward(forward.clone()).await;
        }

        let mut session = client::connect_stream(Arc::new(config), stream, handler)
            .await
            .map_err(|e| SshToolError::SshConnectionFailed(e.to_string()))?;

        match auth_method {
            AuthMethod::Password => {
                let pw = password.ok_or_else(|| {
                    SshToolError::AuthenticationFailed("Password required".to_string())
                })?;
                let auth_res = session
                    .authenticate_password(username, pw)
                    .await
                    .map_err(|e| SshToolError::AuthenticationFailed(e.to_string()))?;
                if !matches!(auth_res, AuthResult::Success) {
                    return Err(SshToolError::AuthenticationFailed(
                        "Password authentication failed".to_string(),
                    ));
                }
            }
            AuthMethod::PublicKey {
                private_key_path,
                passphrase_required,
            } => {
                let expanded = crate::utils::path::expand_tilde(private_key_path);
                let key_data = tokio::fs::read_to_string(&expanded)
                    .await
                    .map_err(|_| SshToolError::KeyFileNotFound(expanded.display().to_string()))?;

                let key = if *passphrase_required {
                    if let Some(pass) = password {
                        PrivateKey::from_openssh(key_data.trim())
                            .map_err(|e| {
                                SshToolError::AuthenticationFailed(format!(
                                    "Failed to load key: {}",
                                    e
                                ))
                            })?
                            .decrypt(pass.as_bytes())
                            .map_err(|e| {
                                SshToolError::AuthenticationFailed(format!(
                                    "Failed to decrypt key: {}",
                                    e
                                ))
                            })?
                    } else {
                        return Err(SshToolError::AuthenticationFailed(
                            "Passphrase required but not provided".to_string(),
                        ));
                    }
                } else {
                    PrivateKey::from_openssh(key_data.trim()).map_err(|e| {
                        SshToolError::AuthenticationFailed(format!("Failed to load key: {}", e))
                    })?
                };

                let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);

                let auth_res = session
                    .authenticate_publickey(username, key_with_alg)
                    .await
                    .map_err(|e| SshToolError::AuthenticationFailed(e.to_string()))?;
                if !matches!(auth_res, AuthResult::Success) {
                    return Err(SshToolError::AuthenticationFailed(
                        "Public key authentication failed".to_string(),
                    ));
                }
            }
        }

        Ok(session)
    }

    /// Connect using configuration
    pub async fn connect(
        connection: &SshConnection,
        password_provider: Option<&str>,
    ) -> Result<SshSession> {
        // If jump hosts are configured, route through them
        if !connection.jump_hosts.is_empty() {
            return Self::connect_via_jump_hosts(
                &connection.jump_hosts,
                connection,
                password_provider,
            )
            .await;
        }

        // Extract remote forwarding configurations from the connection
        use crate::models::ForwardingConfig;
        let remote_forwards: Vec<RemoteForwarding> = connection
            .forwarding_configs
            .iter()
            .filter_map(|config| {
                if let ForwardingConfig::Remote(remote) = config {
                    Some(remote.clone())
                } else {
                    None
                }
            })
            .collect();

        match &connection.auth_method {
            AuthMethod::Password => {
                let password = password_provider.ok_or_else(|| {
                    SshToolError::AuthenticationFailed("Password required".to_string())
                })?;

                Self::connect_password(
                    &connection.host,
                    connection.port,
                    &connection.username,
                    password,
                    connection.host_key_fingerprint.clone(),
                    connection.verify_host_key,
                    remote_forwards,
                )
                .await
            }
            AuthMethod::PublicKey {
                private_key_path,
                passphrase_required,
            } => {
                let passphrase = if *passphrase_required {
                    password_provider
                } else {
                    None
                };

                Self::connect_pubkey(
                    &connection.host,
                    connection.port,
                    &connection.username,
                    private_key_path,
                    passphrase,
                    connection.host_key_fingerprint.clone(),
                    connection.verify_host_key,
                    remote_forwards,
                )
                .await
            }
        }
    }

    /// Connect via jump hosts (ProxyJump) using SSH-over-SSH tunneling.
    ///
    /// Chains through each jump host by opening a direct-tcpip channel on the
    /// current session, wrapping it as a stream, and establishing a new SSH
    /// session on top. Each jump host uses its own credentials (`JumpHost.password`).
    /// The final destination uses `password_provider` passed from the CLI.
    async fn connect_via_jump_hosts(
        jump_hosts: &[JumpHost],
        destination: &SshConnection,
        password_provider: Option<&str>,
    ) -> Result<SshSession> {
        tracing::info!("Connecting via {} jump host(s)", jump_hosts.len());

        // Connect to the first jump host directly
        let first_jump = &jump_hosts[0];
        let first_password = first_jump.password.as_deref();

        let mut current_session = match &first_jump.auth_method {
            AuthMethod::Password => {
                let pw = first_password.ok_or_else(|| {
                    SshToolError::AuthenticationFailed(format!(
                        "Password required for jump host {} (set it in the jump host config)",
                        first_jump.host
                    ))
                })?;
                Self::connect_password(
                    &first_jump.host,
                    first_jump.port,
                    &first_jump.username,
                    pw,
                    first_jump.host_key_fingerprint.clone(),
                    first_jump.verify_host_key,
                    Vec::new(),
                )
                .await?
            }
            AuthMethod::PublicKey {
                private_key_path,
                passphrase_required,
            } => {
                let passphrase = if *passphrase_required {
                    first_password
                } else {
                    None
                };
                Self::connect_pubkey(
                    &first_jump.host,
                    first_jump.port,
                    &first_jump.username,
                    private_key_path,
                    passphrase,
                    first_jump.host_key_fingerprint.clone(),
                    first_jump.verify_host_key,
                    Vec::new(),
                )
                .await?
            }
        };

        tracing::info!("Connected to jump host 1: {}", first_jump.host);

        // Chain through remaining jump hosts, each using its own password
        for (i, jump) in jump_hosts.iter().enumerate().skip(1) {
            let channel = current_session
                .channel_open_direct_tcpip(&jump.host, jump.port as u32, "localhost", 0)
                .await
                .map_err(|e| {
                    SshToolError::SshConnectionFailed(format!(
                        "Failed to open tunnel to jump host {}: {}",
                        jump.host, e
                    ))
                })?;

            let stream = channel_to_stream(channel);
            current_session = Self::connect_over_stream(
                stream,
                &jump.username,
                &jump.auth_method,
                jump.password.as_deref(),
                jump.host_key_fingerprint.clone(),
                jump.verify_host_key,
                Vec::new(),
            )
            .await?;

            tracing::info!("Connected to jump host {}: {}", i + 1, jump.host);
        }

        // Finally, open a channel to the destination through the last jump host
        let channel = current_session
            .channel_open_direct_tcpip(&destination.host, destination.port as u32, "localhost", 0)
            .await
            .map_err(|e| {
                SshToolError::SshConnectionFailed(format!(
                    "Failed to open tunnel to destination {}:{}: {}",
                    destination.host, destination.port, e
                ))
            })?;

        tracing::info!(
            "Opened tunnel to destination {}:{}",
            destination.host,
            destination.port
        );

        // Extract remote forwarding configurations for the destination
        use crate::models::ForwardingConfig;
        let remote_forwards: Vec<RemoteForwarding> = destination
            .forwarding_configs
            .iter()
            .filter_map(|config| {
                if let ForwardingConfig::Remote(remote) = config {
                    Some(remote.clone())
                } else {
                    None
                }
            })
            .collect();

        // Destination uses the CLI-provided password, not jump host passwords
        let stream = channel_to_stream(channel);
        Self::connect_over_stream(
            stream,
            &destination.username,
            &destination.auth_method,
            password_provider,
            destination.host_key_fingerprint.clone(),
            destination.verify_host_key,
            remote_forwards,
        )
        .await
    }

    /// Execute a command on the remote server
    #[allow(dead_code)]
    pub async fn execute_command(
        session: &mut SshSession,
        command: &str,
    ) -> Result<(String, String)> {
        let mut channel = session
            .channel_open_session()
            .await
            .map_err(|e| SshToolError::SshConnectionFailed(e.to_string()))?;

        channel
            .exec(true, command)
            .await
            .map_err(|e| SshToolError::SshConnectionFailed(e.to_string()))?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        loop {
            match channel.wait().await {
                Some(ChannelMsg::Data { ref data }) => {
                    stdout.push_str(&String::from_utf8_lossy(data));
                }
                Some(ChannelMsg::ExtendedData { ref data, .. }) => {
                    stderr.push_str(&String::from_utf8_lossy(data));
                }
                Some(ChannelMsg::Eof) | Some(ChannelMsg::ExitStatus { .. }) => {
                    break;
                }
                Some(ChannelMsg::Close) => break,
                None => break,
                _ => {}
            }
        }

        Ok((stdout, stderr))
    }

    /// Disconnect from SSH server
    pub async fn disconnect(session: &mut SshSession) -> Result<()> {
        session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .map_err(|e| SshToolError::SshConnectionFailed(format!("Disconnect failed: {}", e)))?;

        tracing::info!("Disconnected from SSH server");
        Ok(())
    }
}

/// SSH client handler with host key verification and remote forwarding support
#[derive(Clone)]
pub struct SshClientHandler {
    /// Whether to verify server host keys
    pub verify_host_keys: bool,
    /// Expected host key fingerprint (SHA256)
    pub expected_fingerprint: Option<String>,
    /// Remote forwarding configurations
    /// Shared across async tasks to handle incoming forwarded connections
    pub remote_forwards: Arc<RwLock<Vec<RemoteForwarding>>>,
}

impl SshClientHandler {
    pub fn new() -> Self {
        Self {
            verify_host_keys: false,
            expected_fingerprint: None,
            remote_forwards: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create handler with host key verification enabled
    pub fn with_verification(expected_fingerprint: Option<String>) -> Self {
        Self {
            verify_host_keys: true,
            expected_fingerprint,
            remote_forwards: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create handler with remote forwarding configurations
    #[allow(dead_code)]
    pub fn with_remote_forwards(remote_forwards: Vec<RemoteForwarding>) -> Self {
        Self {
            verify_host_keys: false,
            expected_fingerprint: None,
            remote_forwards: Arc::new(RwLock::new(remote_forwards)),
        }
    }

    /// Add a remote forward configuration
    pub async fn add_remote_forward(&self, forward: RemoteForwarding) {
        let mut forwards = self.remote_forwards.write().await;
        forwards.push(forward);
    }

    /// Clear all remote forward configurations
    #[allow(dead_code)]
    pub async fn clear_remote_forwards(&self) {
        let mut forwards = self.remote_forwards.write().await;
        forwards.clear();
    }

    /// Calculate SHA256 fingerprint of a public key
    fn calculate_fingerprint(key: &PublicKey) -> String {
        // In russh 0.55.0, PublicKey has fingerprint() method
        use russh::keys::ssh_key::HashAlg;
        let fingerprint = key.fingerprint(HashAlg::Sha256);
        fingerprint.to_string()
    }
}

impl Default for SshClientHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    // In russh 0.55.0, check_server_key uses impl Future, no #[async_trait] needed
    fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> impl std::future::Future<Output = std::result::Result<bool, Self::Error>> + Send {
        let fingerprint = Self::calculate_fingerprint(server_public_key);
        let verify_host_keys = self.verify_host_keys;
        let expected_fingerprint = self.expected_fingerprint.clone();

        async move {
            tracing::info!("Server key fingerprint: {}", fingerprint);

            if !verify_host_keys {
                tracing::warn!(
                    "Host key verification disabled - accepting server key without verification"
                );
                tracing::warn!("This is insecure and should only be used for testing!");
                return Ok(true);
            }

            if let Some(expected) = &expected_fingerprint {
                if &fingerprint == expected {
                    tracing::info!("Server key verified successfully");
                    Ok(true)
                } else {
                    tracing::error!("Server key mismatch!");
                    tracing::error!("Expected: {}", expected);
                    tracing::error!("Received: {}", fingerprint);
                    Err(russh::Error::UnknownKey)
                }
            } else {
                tracing::error!(
                    "Host key verification is enabled but no expected fingerprint is configured"
                );
                tracing::error!("Server key fingerprint: {}", fingerprint);
                tracing::error!(
                    "Add this fingerprint to your connection config to allow this connection"
                );
                Err(russh::Error::UnknownKey)
            }
        }
    }

    // Handle remote port forwarding (-R) connections from the server
    fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: Channel<Msg>,
        connected_address: &str,
        connected_port: u32,
        originator_address: &str,
        originator_port: u32,
        _session: &mut client::Session,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send {
        let connected_address = connected_address.to_string();
        let originator_address = originator_address.to_string();
        let remote_forwards = self.remote_forwards.clone();

        async move {
            tracing::info!(
                "Received forwarded connection from {}:{} to {}:{}",
                originator_address,
                originator_port,
                connected_address,
                connected_port
            );

            // Find matching remote forward configuration
            let forwards = remote_forwards.read().await;
            let forward_config = forwards
                .iter()
                .find(|f| f.remote_port == connected_port as u16)
                .cloned();
            drop(forwards);

            match forward_config {
                Some(config) => {
                    // Spawn task to handle this connection
                    let local_addr = format!("{}:{}", config.local_host, config.local_port);
                    tracing::info!(
                        "Forwarding remote:{}  to local {}",
                        connected_port,
                        local_addr
                    );

                    // Connect to local service
                    match tokio::net::TcpStream::connect(&local_addr).await {
                        Ok(local_stream) => {
                            tracing::debug!("Connected to local service {}", local_addr);

                            // Start bidirectional forwarding
                            tokio::spawn(async move {
                                if let Err(e) =
                                    Self::forward_bidirectional(channel, local_stream).await
                                {
                                    tracing::error!(
                                        "Remote forward bidirectional transfer failed: {}",
                                        e
                                    );
                                }
                            });

                            Ok(())
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to connect to local service {}: {}",
                                local_addr,
                                e
                            );
                            Err(russh::Error::Disconnect)
                        }
                    }
                }
                None => {
                    tracing::warn!(
                        "No remote forward configuration found for port {}",
                        connected_port
                    );
                    Err(russh::Error::Disconnect)
                }
            }
        }
    }
}

impl SshClientHandler {
    /// Forward data bidirectionally between SSH channel and local TCP stream
    async fn forward_bidirectional(
        mut channel: Channel<Msg>,
        local_stream: tokio::net::TcpStream,
    ) -> std::result::Result<(), russh::Error> {
        let (mut local_read, mut local_write) = tokio::io::split(local_stream);

        // Buffer for reading data
        let mut buf = vec![0u8; 8192];

        loop {
            tokio::select! {
                // Read from local stream and write to SSH channel
                result = local_read.read(&mut buf) => {
                    match result {
                        Ok(0) => {
                            // Local connection closed
                            tracing::debug!("Local connection closed (EOF)");
                            let _ = channel.eof().await;
                            break;
                        }
                        Ok(n) => {
                            // Send data to remote through SSH channel
                            if let Err(e) = channel.data(&buf[..n]).await {
                                tracing::error!("Failed to send data to SSH channel: {}", e);
                                return Err(e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to read from local stream: {}", e);
                            return Err(russh::Error::IO(e));
                        }
                    }
                }

                // Read from SSH channel and write to local stream
                result = channel.wait() => {
                    match result {
                        Some(ChannelMsg::Data { ref data }) => {
                            // Write data to local stream
                            if let Err(e) = local_write.write_all(data).await {
                                tracing::error!("Failed to write to local stream: {}", e);
                                return Err(russh::Error::IO(e));
                            }
                        }
                        Some(ChannelMsg::Eof) => {
                            // SSH channel closed
                            tracing::debug!("SSH channel closed (EOF)");
                            break;
                        }
                        Some(ChannelMsg::Close) => {
                            // SSH channel closed
                            tracing::debug!("SSH channel closed");
                            break;
                        }
                        Some(_) => {
                            // Ignore other messages
                        }
                        None => {
                            // Channel stream ended
                            tracing::debug!("SSH channel stream ended");
                            break;
                        }
                    }
                }
            }
        }

        tracing::debug!("Bidirectional forwarding completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_client_handler() {
        let _handler = SshClientHandler::new();
        // Just ensure it can be created
        assert!(true);
    }

    // Note: Integration tests for actual SSH connections would require a test SSH server
    // Those should be in integration tests with proper setup
}
