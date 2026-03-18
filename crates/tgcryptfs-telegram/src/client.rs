use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use grammers_client::types::Downloadable;
use grammers_client::{Client, Config, InputMessage, SignInError};
use grammers_session::Session;
use tokio::sync::{Mutex, Semaphore};

use crate::error::{Result, TelegramError};
use crate::types::{AuthState, DownloadResult, TelegramConfig, TransportStats, UploadResult};

/// Trait abstracting Telegram transport operations.
/// This allows mocking in tests and potentially swapping backends.
#[async_trait::async_trait]
pub trait BlockTransport: Send + Sync {
    /// Upload encrypted block data, returns the message ID.
    async fn upload_block(&self, data: &[u8], filename: &str) -> Result<UploadResult>;

    /// Download block data by message ID.
    async fn download_block(&self, message_id: i64) -> Result<DownloadResult>;

    /// Delete a message (block) by ID.
    async fn delete_message(&self, message_id: i64) -> Result<()>;

    /// Delete multiple messages in batch.
    async fn delete_messages(&self, message_ids: &[i64]) -> Result<()>;

    /// Check if connected and authenticated.
    fn is_connected(&self) -> bool;
}

/// Production Telegram client using grammers.
pub struct TelegramClient {
    config: TelegramConfig,
    upload_semaphore: Arc<Semaphore>,
    download_semaphore: Arc<Semaphore>,
    stats: Arc<TransportStatsInner>,
    connected: std::sync::atomic::AtomicBool,
    /// The actual grammers client, set after connect().
    inner: Mutex<Option<Client>>,
    /// Cached password token from sign_in() for 2FA flow.
    password_token: Mutex<Option<grammers_client::types::PasswordToken>>,
}

struct TransportStatsInner {
    blocks_uploaded: AtomicU64,
    blocks_downloaded: AtomicU64,
    bytes_uploaded: AtomicU64,
    bytes_downloaded: AtomicU64,
    upload_errors: AtomicU64,
    download_errors: AtomicU64,
    messages_deleted: AtomicU64,
}

impl TransportStatsInner {
    fn new() -> Self {
        Self {
            blocks_uploaded: AtomicU64::new(0),
            blocks_downloaded: AtomicU64::new(0),
            bytes_uploaded: AtomicU64::new(0),
            bytes_downloaded: AtomicU64::new(0),
            upload_errors: AtomicU64::new(0),
            download_errors: AtomicU64::new(0),
            messages_deleted: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> TransportStats {
        TransportStats {
            blocks_uploaded: self.blocks_uploaded.load(Ordering::Relaxed),
            blocks_downloaded: self.blocks_downloaded.load(Ordering::Relaxed),
            bytes_uploaded: self.bytes_uploaded.load(Ordering::Relaxed),
            bytes_downloaded: self.bytes_downloaded.load(Ordering::Relaxed),
            upload_errors: self.upload_errors.load(Ordering::Relaxed),
            download_errors: self.download_errors.load(Ordering::Relaxed),
            messages_deleted: self.messages_deleted.load(Ordering::Relaxed),
        }
    }
}

impl TelegramClient {
    /// Create a new Telegram client with the given config.
    /// Does NOT connect - call `connect()` separately.
    pub fn new(config: TelegramConfig) -> Self {
        let upload_semaphore = Arc::new(Semaphore::new(config.max_concurrent_uploads));
        let download_semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads));

        Self {
            config,
            upload_semaphore,
            download_semaphore,
            stats: Arc::new(TransportStatsInner::new()),
            connected: std::sync::atomic::AtomicBool::new(false),
            inner: Mutex::new(None),
            password_token: Mutex::new(None),
        }
    }

    /// Connect to Telegram and restore session if available.
    pub async fn connect(&self) -> Result<AuthState> {
        tracing::info!("Connecting to Telegram...");

        let session = Session::load_file_or_create(&self.config.session_path)
            .map_err(|e| TelegramError::Session(format!("load session: {e}")))?;

        let client = Client::connect(Config {
            session,
            api_id: self.config.api_id,
            api_hash: self.config.api_hash.clone(),
            params: Default::default(),
        })
        .await
        .map_err(|e| TelegramError::Api(format!("connect: {e}")))?;

        let authorized = client
            .is_authorized()
            .await
            .map_err(|e| TelegramError::Api(format!("auth check: {e}")))?;

        if authorized {
            self.connected.store(true, Ordering::SeqCst);
            *self.inner.lock().await = Some(client);
            tracing::info!("Telegram: already authenticated");
            Ok(AuthState::Authenticated)
        } else {
            *self.inner.lock().await = Some(client);
            tracing::info!("Telegram: authentication required");
            Ok(AuthState::NotAuthenticated)
        }
    }

    /// Request a login code to be sent to the phone number.
    pub async fn request_login_code(&self, phone: &str) -> Result<LoginToken> {
        let guard = self.inner.lock().await;
        let client = guard.as_ref().ok_or(TelegramError::NotConnected)?;

        let token = client
            .request_login_code(phone)
            .await
            .map_err(|e| TelegramError::Api(format!("request code: {e}")))?;

        Ok(LoginToken { inner: token })
    }

    /// Sign in with the verification code.
    pub async fn sign_in(&self, token: &LoginToken, code: &str) -> Result<AuthState> {
        let guard = self.inner.lock().await;
        let client = guard.as_ref().ok_or(TelegramError::NotConnected)?;

        match client.sign_in(&token.inner, code).await {
            Ok(_user) => {
                self.connected.store(true, Ordering::SeqCst);
                self.save_session_inner(client).await?;
                Ok(AuthState::Authenticated)
            }
            Err(SignInError::PasswordRequired(pwd_token)) => {
                *self.password_token.lock().await = Some(pwd_token);
                Ok(AuthState::AwaitingPassword)
            }
            Err(SignInError::InvalidCode) => {
                Err(TelegramError::Api("invalid verification code".into()))
            }
            Err(e) => Err(TelegramError::Api(format!("sign in: {e}"))),
        }
    }

    /// Check 2FA password using the cached password token from sign_in().
    pub async fn check_password(&self, password: &str) -> Result<AuthState> {
        let guard = self.inner.lock().await;
        let client = guard.as_ref().ok_or(TelegramError::NotConnected)?;

        let token = self.password_token.lock().await.take().ok_or_else(|| {
            TelegramError::Api("no cached password token (call sign_in first)".into())
        })?;

        match client.check_password(token, password).await {
            Ok(_user) => {
                self.connected.store(true, Ordering::SeqCst);
                self.save_session_inner(client).await?;
                Ok(AuthState::Authenticated)
            }
            Err(e) => Err(TelegramError::Api(format!(
                "2FA password check failed: {e}"
            ))),
        }
    }

    /// Save the session to disk.
    async fn save_session_inner(&self, client: &Client) -> Result<()> {
        client
            .session()
            .save_to_file(&self.config.session_path)
            .map_err(|e| TelegramError::Session(format!("save session: {e}")))?;
        Ok(())
    }

    /// Save the current session.
    pub async fn save_session(&self) -> Result<()> {
        let guard = self.inner.lock().await;
        if let Some(client) = guard.as_ref() {
            self.save_session_inner(client).await
        } else {
            Ok(())
        }
    }

    /// Get current transport statistics.
    pub fn stats(&self) -> TransportStats {
        self.stats.snapshot()
    }

    /// Get the config.
    pub fn config(&self) -> &TelegramConfig {
        &self.config
    }

    fn validate_upload_size(&self, size: usize) -> Result<()> {
        if size > self.config.max_upload_size {
            return Err(TelegramError::BlockTooLarge {
                size,
                max: self.config.max_upload_size,
            });
        }
        Ok(())
    }

    fn ensure_connected(&self) -> Result<()> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(TelegramError::NotConnected);
        }
        Ok(())
    }
}

/// Opaque login token wrapper.
pub struct LoginToken {
    inner: grammers_client::types::LoginToken,
}

#[async_trait::async_trait]
impl BlockTransport for TelegramClient {
    async fn upload_block(&self, data: &[u8], filename: &str) -> Result<UploadResult> {
        self.ensure_connected()?;
        self.validate_upload_size(data.len())?;

        let _permit = self
            .upload_semaphore
            .acquire()
            .await
            .map_err(|_| TelegramError::Upload("semaphore closed".into()))?;

        let mut last_err = None;
        for attempt in 0..self.config.max_retries {
            match self.do_upload(data, filename).await {
                Ok(result) => {
                    self.stats.blocks_uploaded.fetch_add(1, Ordering::Relaxed);
                    self.stats
                        .bytes_uploaded
                        .fetch_add(data.len() as u64, Ordering::Relaxed);
                    return Ok(result);
                }
                Err(e) => {
                    self.stats.upload_errors.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(attempt, error = %e, "upload attempt failed");
                    last_err = Some(e);
                    if attempt + 1 < self.config.max_retries {
                        let delay = self.config.retry_base_delay_ms * 2u64.pow(attempt);
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(last_err.unwrap_or(TelegramError::Upload("all retries exhausted".into())))
    }

    async fn download_block(&self, message_id: i64) -> Result<DownloadResult> {
        self.ensure_connected()?;

        let _permit = self
            .download_semaphore
            .acquire()
            .await
            .map_err(|_| TelegramError::Download("semaphore closed".into()))?;

        let mut last_err = None;
        for attempt in 0..self.config.max_retries {
            match self.do_download(message_id).await {
                Ok(result) => {
                    self.stats.blocks_downloaded.fetch_add(1, Ordering::Relaxed);
                    self.stats
                        .bytes_downloaded
                        .fetch_add(result.data.len() as u64, Ordering::Relaxed);
                    return Ok(result);
                }
                Err(e) => {
                    self.stats.download_errors.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(attempt, error = %e, "download attempt failed");
                    last_err = Some(e);
                    if attempt + 1 < self.config.max_retries {
                        let delay = self.config.retry_base_delay_ms * 2u64.pow(attempt);
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(last_err.unwrap_or(TelegramError::Download("all retries exhausted".into())))
    }

    async fn delete_message(&self, message_id: i64) -> Result<()> {
        self.ensure_connected()?;
        self.do_delete(&[message_id]).await?;
        self.stats.messages_deleted.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn delete_messages(&self, message_ids: &[i64]) -> Result<()> {
        self.ensure_connected()?;
        self.do_delete(message_ids).await?;
        self.stats
            .messages_deleted
            .fetch_add(message_ids.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

impl TelegramClient {
    async fn do_upload(&self, data: &[u8], filename: &str) -> Result<UploadResult> {
        let guard = self.inner.lock().await;
        let client = guard.as_ref().ok_or(TelegramError::NotConnected)?;

        // Upload file as a stream
        let mut stream = std::io::Cursor::new(data);
        let uploaded = client
            .upload_stream(&mut stream, data.len(), filename.to_string())
            .await
            .map_err(|e| TelegramError::Upload(format!("upload stream: {e}")))?;

        // Send to Saved Messages (self)
        let me = client
            .get_me()
            .await
            .map_err(|e| TelegramError::Api(format!("get_me: {e}")))?;

        let message = client
            .send_message(&me, InputMessage::text("").document(uploaded))
            .await
            .map_err(|e| TelegramError::Upload(format!("send document: {e}")))?;

        Ok(UploadResult {
            message_id: i64::from(message.id()),
            size: data.len(),
        })
    }

    async fn do_download(&self, message_id: i64) -> Result<DownloadResult> {
        let guard = self.inner.lock().await;
        let client = guard.as_ref().ok_or(TelegramError::NotConnected)?;

        let me = client
            .get_me()
            .await
            .map_err(|e| TelegramError::Api(format!("get_me: {e}")))?;

        // Search for the message by ID in saved messages
        let mut messages = client.iter_messages(&me);
        let mut target_msg = None;

        // We need to find the specific message - iterate until we find it
        // grammers doesn't have a direct get-by-id for saved messages
        while let Some(msg) = messages
            .next()
            .await
            .map_err(|e| TelegramError::Download(format!("iter messages: {e}")))?
        {
            if i64::from(msg.id()) == message_id {
                target_msg = Some(msg);
                break;
            }
            // Don't iterate forever - messages are in reverse chronological order
            // If we've gone past the ID, stop
            if i64::from(msg.id()) < message_id {
                break;
            }
        }

        let msg = target_msg.ok_or(TelegramError::MessageNotFound(message_id))?;

        // Download the media
        let media = msg
            .media()
            .ok_or(TelegramError::Download("message has no media".into()))?;

        let mut data = Vec::new();
        let downloadable = Downloadable::Media(media);
        let mut download = client.iter_download(&downloadable);
        while let Some(chunk) = download
            .next()
            .await
            .map_err(|e| TelegramError::Download(format!("download chunk: {e}")))?
        {
            data.extend_from_slice(&chunk);
        }

        Ok(DownloadResult { data, message_id })
    }

    async fn do_delete(&self, message_ids: &[i64]) -> Result<()> {
        let guard = self.inner.lock().await;
        let client = guard.as_ref().ok_or(TelegramError::NotConnected)?;

        let me = client
            .get_me()
            .await
            .map_err(|e| TelegramError::Api(format!("get_me: {e}")))?;

        let ids: Vec<i32> = message_ids.iter().map(|&id| id as i32).collect();
        client
            .delete_messages(&me, &ids)
            .await
            .map_err(|e| TelegramError::Delete(format!("delete: {e}")))?;

        Ok(())
    }
}
