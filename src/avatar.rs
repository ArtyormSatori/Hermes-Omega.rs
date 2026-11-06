//! # Omega Realtime Avatar Engine (`src/avatar.rs`)
//!
//! Cơ chế **Realtime Avatar** của **Hermes Omega.rs**:
//! - 5 trạng thái nhận thức thời gian thực: `Idle` (Đang chờ), `Listening` (Đang lắng nghe),
//!   `Thinking` (Đang suy luận ReAct), `Speaking` (Đang phát âm đồng bộ khẩu hình), `Executing` (Đang chạy Sandbox).
//! - Tự động phân tích âm vị Tiếng Việt sang khẩu hình (Viseme: `Closed`, `HalfOpen`, `WideOpen`, `Rounded`).
//! - Hỗ trợ kết xuất kép: **Dấu thương hiệu Omega ASCII trên TUI** và **Sự kiện WebSocket 60fps** trên Dashboard.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmegaAvatarState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Executing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouthViseme {
    Closed,   // m, b, p, im lặng
    HalfOpen, // e, ê, i, ư, ơ
    WideOpen, // a, ă, â
    Rounded,  // o, ô, u
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmegaAvatarFrame {
    pub state: OmegaAvatarState,
    pub viseme: MouthViseme,
    pub blinking: bool,
    pub voice_energy_db: f32,
    pub status_label_vi: String,
    pub ascii_portrait: String,
}

pub struct OmegaAvatarController {
    pub state: OmegaAvatarState,
    pub tick_counter: u64,
}

impl Default for OmegaAvatarController {
    fn default() -> Self {
        Self {
            state: OmegaAvatarState::Idle,
            tick_counter: 0,
        }
    }
}

impl OmegaAvatarController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_state(&mut self, state: OmegaAvatarState) {
        self.state = state;
    }

    /// Phân tích ký tự Tiếng Việt đang phát âm thành khẩu hình (Viseme) tương ứng
    pub fn phoneme_to_viseme(ch: char) -> MouthViseme {
        let lower = ch.to_lowercase().next().unwrap_or(' ');
        match lower {
            'a' | 'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'â' => MouthViseme::WideOpen,
            'o' | 'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'u' | 'ù' | 'ú' => MouthViseme::Rounded,
            'e' | 'ê' | 'i' | 'ư' | 'ơ' | 'y' => MouthViseme::HalfOpen,
            _ => MouthViseme::Closed,
        }
    }

    /// Sinh khung hình Avatar kế tiếp (cho cả TUI ASCII và WebSocket Dashboard)
    pub fn next_frame(&mut self, speaking_char: Option<char>) -> OmegaAvatarFrame {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        let blinking = self.tick_counter % 19 == 0;

        let viseme = match (self.state, speaking_char) {
            (OmegaAvatarState::Speaking, Some(c)) => Self::phoneme_to_viseme(c),
            (OmegaAvatarState::Speaking, None) => {
                if self.tick_counter % 2 == 0 {
                    MouthViseme::HalfOpen
                } else {
                    MouthViseme::Closed
                }
            }
            _ => MouthViseme::Closed,
        };

        let (status_label_vi, energy) = match self.state {
            OmegaAvatarState::Idle => ("Đang chờ lệnh (Idle)", 0.05),
            OmegaAvatarState::Listening => ("Đang lắng nghe âm thanh (Listening)", 0.45),
            OmegaAvatarState::Thinking => ("Đang suy luận ReAct (Thinking)", 0.25),
            OmegaAvatarState::Speaking => ("Đang phản hồi giọng nói (Speaking)", 0.85),
            OmegaAvatarState::Executing => ("Đang thực thi Sandbox (Executing)", 0.60),
        };

        // Không vẽ khuôn mặt nữa — chỉ giữ dấu thương hiệu Omega.
        let ascii_portrait = "   .───────────.\n\
             .─'  .:ΩΩΩΩ:.   '─.\n\
            │                   │\n\
            │      Ω OMEGA      │\n\
            │                   │\n\
             '─.             .─'\n\
                '───────────'"
            .to_string();

        OmegaAvatarFrame {
            state: self.state,
            viseme,
            blinking,
            voice_energy_db: energy,
            status_label_vi: status_label_vi.to_string(),
            ascii_portrait,
        }
    }
}
