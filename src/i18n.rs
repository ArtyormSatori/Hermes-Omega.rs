//! # Hệ thống Đa ngôn ngữ & Xử lý Tiếng Việt Chuẩn (`src/i18n.rs`)
//!
//! Cung cấp:
//! 1. Bản địa hoá Tiếng Việt chuẩn (`Locale::Vi` - mặc định) và Tiếng Anh (`Locale::En`) cho TUI, CLI, Gateway.
//! 2. Chuẩn hoá dấu Tiếng Việt (`normalize_vietnamese_diacritics`) phục vụ tìm kiếm toàn văn FTS5 không phụ thuộc dấu.
//! 3. Cắt chuỗi an toàn theo ranh giới ký tự UTF-8 (`safe_truncate_utf8`), khắc phục triệt để lỗi panic khi cắt byte tiếng Việt.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Vi,
    En,
}

impl Default for Locale {
    fn default() -> Self {
        Locale::Vi
    }
}

impl Locale {
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_lowercase().as_str() {
            "en" | "eng" | "english" => Locale::En,
            _ => Locale::Vi,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Locale::Vi => "Tiếng Việt (Chuẩn)",
            Locale::En => "English",
        }
    }
}

/// Cắt chuỗi UTF-8 an toàn theo số lượng ký tự Unicode (tránh lỗi byte boundary panic với chữ có dấu Tiếng Việt)
pub fn safe_truncate_utf8(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// Chuẩn hoá chuỗi Tiếng Việt có dấu về dạng không dấu (phục vụ FTS5 & so khớp mờ)
pub fn normalize_vietnamese_diacritics(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        let mapped = match c {
            'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ'
            | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'À' | 'Á' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ằ' | 'Ắ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ầ' | 'Ấ'
            | 'Ẩ' | 'Ẫ' | 'Ậ' => 'A',
            'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => 'e',
            'È' | 'É' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ề' | 'Ế' | 'Ể' | 'Ễ' | 'Ệ' => 'E',
            'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'Ì' | 'Í' | 'Ỉ' | 'Ĩ' | 'Ị' => 'I',
            'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ'
            | 'ở' | 'ỡ' | 'ợ' => 'o',
            'Ò' | 'Ó' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ồ' | 'Ố' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ờ' | 'Ớ'
            | 'Ở' | 'Ỡ' | 'Ợ' => 'O',
            'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => 'u',
            'Ù' | 'Ú' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ử' | 'Ữ' | 'Ự' => 'U',
            'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'Ỳ' | 'Ý' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'Y',
            'đ' => 'd',
            'Đ' => 'D',
            other => other,
        };
        out.push(mapped);
    }
    out
}

pub struct I18n;

impl I18n {
    pub fn t(locale: Locale, key: &str) -> &'static str {
        match (locale, key) {
            (Locale::Vi, "app_banner") => "Ω Hermes Omega.rs — Runtime Agent Tự Trị Hợp Nhất bằng Rust",
            (Locale::En, "app_banner") => "Ω Hermes Omega.rs — Unified Autonomous Rust Agent Runtime",

            (Locale::Vi, "tui_help") => "Phím tắt: [F2] Đổi giao diện | [F3] Ngôn ngữ VI/EN | [/upload <path>] Tải file >1GB | [/quit] Thoát",
            (Locale::En, "tui_help") => "Hotkeys: [F2] Cycle Theme | [F3] Toggle VI/EN | [/upload <path>] >1GB Stream | [/quit] Quit",

            (Locale::Vi, "doctor_header") => "=== [Báo cáo Kiểm định Hệ thống Ω Hermes Omega.rs] ===",
            (Locale::En, "doctor_header") => "=== [Ω Hermes Omega.rs System Doctor Report] ===",

            (Locale::Vi, "transfer_ready") => "Luồng truyền tải >1GB sẵn sàng (Zero-Copy BufWriter 8MB, tối đa 16GB/file)",
            (Locale::En, "transfer_ready") => ">1GB Streaming Engine Ready (Zero-Copy 8MB BufWriter, max 16GB/file)",

            _ => "Ω Hermes Omega.rs",
        }
    }
}
