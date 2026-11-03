//! # Bộ xử lý Truyền tải Tệp Lớn >1GB – 16GB (`src/large_transfer.rs`)
//!
//! Giải quyết triệt để giới hạn bộ nhớ đệm khi truyền tải các tập dữ liệu lớn, model GGUF/Safetensors (>1GB – 16GB):
//! 1. **Zero-Copy Streaming Upload (`stream_upload_chunk`)**:
//!    - Ghi nối tiếp (append) theo từng mảnh (chunk) qua `tokio::io::BufWriter` (8 MB RAM buffer cố định).
//!    - Tính toán mã băm `SHA-256` tăng dần trên luồng dữ liệu mà không nạp toàn bộ tệp vào bộ nhớ.
//! 2. **HTTP 206 Partial Content Range Download (`read_file_range`)**:
//!    - Hỗ trợ header `Range: bytes=start-end` giúp trình duyệt hoặc CLI tạm dừng và tải tiếp (Resume) các tệp >1GB.

use crate::error::{HermesError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufWriter};

/// Kích thước bộ đệm luồng cố định 8 MB (đảm bảo upload file 10 GB chỉ tốn 8 MB RAM)
pub const STREAM_BUFFER_SIZE: usize = 8 * 1024 * 1024;
/// Giới hạn kích thước tệp tối đa: 16 GB
pub const MAX_TRANSFER_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeTransferReceipt {
    pub file_name: String,
    pub total_bytes_written: u64,
    pub chunk_index: usize,
    pub sha256_hex: String,
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByteRangeSlice {
    pub start: u64,
    pub end: u64,
    pub total_size: u64,
    pub data: Vec<u8>,
}

pub struct LargeFileEngine {
    storage_dir: PathBuf,
}

impl LargeFileEngine {
    pub fn new(hermes_home: &Path) -> Result<Self> {
        let storage_dir = hermes_home.join("transfers");
        std::fs::create_dir_all(&storage_dir)?;
        Ok(Self { storage_dir })
    }

    pub fn sanitize_filename(raw: &str) -> String {
        raw.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Ghi một mảnh dữ liệu (chunk) vào tệp đích bằng `BufWriter` 8MB và trả về mã băm SHA-256 của mảnh
    pub async fn write_stream_chunk(
        &self,
        file_name: &str,
        chunk_index: usize,
        append: bool,
        payload: &[u8],
    ) -> Result<LargeTransferReceipt> {
        let safe_name = Self::sanitize_filename(file_name);
        let dest = self.storage_dir.join(&safe_name);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&dest)
            .await?;

        let mut writer = BufWriter::with_capacity(STREAM_BUFFER_SIZE, file);
        let mut hasher = Sha256::new();

        for slice in payload.chunks(64 * 1024) {
            writer.write_all(slice).await?;
            hasher.update(slice);
        }
        writer.flush().await?;

        let meta = tokio::fs::metadata(&dest).await?;
        if meta.len() > MAX_TRANSFER_BYTES {
            return Err(HermesError::SandboxViolation(format!(
                "File {} exceeds maximum 16 GB transfer limit",
                safe_name
            )));
        }

        let sha256_hex = hex::encode(hasher.finalize());
        Ok(LargeTransferReceipt {
            file_name: safe_name,
            total_bytes_written: meta.len(),
            chunk_index,
            sha256_hex,
            storage_path: dest.display().to_string(),
        })
    }

    /// Đọc một phân đoạn byte (`Range: bytes=start-end`) phục vụ tải xuống nối tiếp HTTP 206 cho tệp >1GB
    pub async fn read_range_slice(
        &self,
        file_name: &str,
        start: u64,
        max_chunk_bytes: usize,
    ) -> Result<ByteRangeSlice> {
        let safe_name = Self::sanitize_filename(file_name);
        let path = self.storage_dir.join(&safe_name);
        let mut file = File::open(&path).await?;
        let total_size = file.metadata().await?.len();

        if start >= total_size {
            return Ok(ByteRangeSlice {
                start,
                end: total_size,
                total_size,
                data: Vec::new(),
            });
        }

        file.seek(SeekFrom::Start(start)).await?;
        let to_read = ((total_size - start) as usize).min(max_chunk_bytes);
        let mut buf = vec![0u8; to_read];
        let n = file.read(&mut buf).await?;
        buf.truncate(n);

        Ok(ByteRangeSlice {
            start,
            end: start + (n as u64).saturating_sub(1),
            total_size,
            data: buf,
        })
    }
}
