use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Mutex;

use crate::client::BlockTransport;
use crate::error::{Result, TelegramError};
use crate::types::{DownloadResult, UploadResult};

/// In-memory mock Telegram transport for testing.
pub struct MockTransport {
    messages: Mutex<HashMap<i64, Vec<u8>>>,
    next_id: AtomicI64,
    connected: AtomicBool,
    /// If set, all uploads will fail with this error message.
    pub fail_uploads: Mutex<Option<String>>,
    /// If set, all downloads will fail with this error message.
    pub fail_downloads: Mutex<Option<String>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            messages: Mutex::new(HashMap::new()),
            next_id: AtomicI64::new(1),
            connected: AtomicBool::new(true),
            fail_uploads: Mutex::new(None),
            fail_downloads: Mutex::new(None),
        }
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }

    pub fn message_count(&self) -> usize {
        self.messages.lock().unwrap().len()
    }

    pub fn get_raw(&self, message_id: i64) -> Option<Vec<u8>> {
        self.messages.lock().unwrap().get(&message_id).cloned()
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BlockTransport for MockTransport {
    async fn upload_block(&self, data: &[u8], _filename: &str) -> Result<UploadResult> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(TelegramError::NotConnected);
        }

        if let Some(ref err) = *self.fail_uploads.lock().unwrap() {
            return Err(TelegramError::Upload(err.clone()));
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.messages.lock().unwrap().insert(id, data.to_vec());

        Ok(UploadResult {
            message_id: id,
            size: data.len(),
        })
    }

    async fn download_block(&self, message_id: i64) -> Result<DownloadResult> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(TelegramError::NotConnected);
        }

        if let Some(ref err) = *self.fail_downloads.lock().unwrap() {
            return Err(TelegramError::Download(err.clone()));
        }

        let messages = self.messages.lock().unwrap();
        match messages.get(&message_id) {
            Some(data) => Ok(DownloadResult {
                data: data.clone(),
                message_id,
            }),
            None => Err(TelegramError::MessageNotFound(message_id)),
        }
    }

    async fn delete_message(&self, message_id: i64) -> Result<()> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(TelegramError::NotConnected);
        }

        let mut messages = self.messages.lock().unwrap();
        if messages.remove(&message_id).is_some() {
            Ok(())
        } else {
            Err(TelegramError::MessageNotFound(message_id))
        }
    }

    async fn delete_messages(&self, message_ids: &[i64]) -> Result<()> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(TelegramError::NotConnected);
        }

        let mut messages = self.messages.lock().unwrap();
        for id in message_ids {
            messages.remove(id);
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upload_and_download() {
        let transport = MockTransport::new();
        let data = vec![0x42; 1024];
        let result = transport.upload_block(&data, "block.srb1").await.unwrap();
        assert_eq!(result.size, 1024);

        let downloaded = transport.download_block(result.message_id).await.unwrap();
        assert_eq!(downloaded.data, data);
    }

    #[tokio::test]
    async fn download_nonexistent() {
        let transport = MockTransport::new();
        let err = transport.download_block(999).await.unwrap_err();
        assert!(matches!(err, TelegramError::MessageNotFound(999)));
    }

    #[tokio::test]
    async fn delete_message() {
        let transport = MockTransport::new();
        let result = transport.upload_block(&[1, 2, 3], "test").await.unwrap();
        assert_eq!(transport.message_count(), 1);

        transport.delete_message(result.message_id).await.unwrap();
        assert_eq!(transport.message_count(), 0);
    }

    #[tokio::test]
    async fn delete_batch() {
        let transport = MockTransport::new();
        let r1 = transport.upload_block(&[1], "a").await.unwrap();
        let r2 = transport.upload_block(&[2], "b").await.unwrap();
        let r3 = transport.upload_block(&[3], "c").await.unwrap();
        assert_eq!(transport.message_count(), 3);

        transport
            .delete_messages(&[r1.message_id, r3.message_id])
            .await
            .unwrap();
        assert_eq!(transport.message_count(), 1);
        assert!(transport.get_raw(r2.message_id).is_some());
    }

    #[tokio::test]
    async fn not_connected() {
        let transport = MockTransport::new();
        transport.set_connected(false);

        let err = transport.upload_block(&[1], "test").await.unwrap_err();
        assert!(matches!(err, TelegramError::NotConnected));

        let err = transport.download_block(1).await.unwrap_err();
        assert!(matches!(err, TelegramError::NotConnected));
    }

    #[tokio::test]
    async fn forced_upload_failure() {
        let transport = MockTransport::new();
        *transport.fail_uploads.lock().unwrap() = Some("disk full".into());

        let err = transport.upload_block(&[1], "test").await.unwrap_err();
        assert!(matches!(err, TelegramError::Upload(_)));
    }

    #[tokio::test]
    async fn multiple_uploads_unique_ids() {
        let transport = MockTransport::new();
        let r1 = transport.upload_block(&[1], "a").await.unwrap();
        let r2 = transport.upload_block(&[2], "b").await.unwrap();
        assert_ne!(r1.message_id, r2.message_id);
    }
}
