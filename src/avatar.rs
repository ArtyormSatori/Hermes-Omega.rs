//! # MUSE.AI Realtime Embodied Avatar Engine (`src/avatar.rs`)
//!
//! Mô phỏng cơ chế **Realtime Avatar** của **MUSE.AI**:
//! - 5 trạng thái nhận thức thời gian thực: `Idle` (Đang chờ), `Listening` (Đang lắng nghe),
//!   `Thinking` (Đang suy luận ReAct), `Speaking` (Đang phát âm đồng bộ khẩu hình), `Executing` (Đang chạy Sandbox).
//! - Tự động phân tích âm vị Tiếng Việt sang khẩu hình (Viseme: `Closed`, `HalfOpen`, `WideOpen`, `Rounded`).
//! - Hỗ trợ kết xuất kép: **Khung chân dung ASCII trực tiếp trên TUI** và **Sự kiện WebSocket 60fps** trên Dashboard.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MuseAvatarState {
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
pub struct MuseAvatarFrame {
    pub state: MuseAvatarState,
    pub viseme: MouthViseme,
    pub blinking: bool,
    pub voice_energy_db: f32,
    pub status_label_vi: String,
    pub ascii_portrait: String,
}

pub struct MuseAvatarController {
    pub state: MuseAvatarState,
    pub tick_counter: u64,
}

impl Default for MuseAvatarController {
    fn default() -> Self {
        Self {
            state: MuseAvatarState::Idle,
            tick_counter: 0,
        }
    }
}

impl MuseAvatarController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_state(&mut self, state: MuseAvatarState) {
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
    pub fn next_frame(&mut self, speaking_char: Option<char>) -> MuseAvatarFrame {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        let blinking = self.tick_counter % 19 == 0;

        let viseme = match (self.state, speaking_char) {
            (MuseAvatarState::Speaking, Some(c)) => Self::phoneme_to_viseme(c),
            (MuseAvatarState::Speaking, None) => {
                if self.tick_counter % 2 == 0 {
                    MouthViseme::HalfOpen
                } else {
                    MouthViseme::Closed
                }
            }
            _ => MouthViseme::Closed,
        };

        let eyes = if blinking {
            "  ─     ─  "
        } else {
            match self.state {
                MuseAvatarState::Idle => "  ◉     ◉  ",
                MuseAvatarState::Listening => "  ◎     ◎  ",
                MuseAvatarState::Thinking => "  ◔     ◔  ",
                MuseAvatarState::Speaking => "  ◉     ◉  ",
                MuseAvatarState::Executing => "  ■     ■  ",
            }
        };

        let mouth = match viseme {
            MouthViseme::Closed => "    ───    ",
            MouthViseme::HalfOpen => "    ═╦═    ",
            MouthViseme::WideOpen => "   ╔═══╗   ",
            MouthViseme::Rounded => "    (○)    ",
        };

        let (status_label_vi, energy) = match self.state {
            MuseAvatarState::Idle => ("Đang chờ lệnh (Idle)", 0.05),
            MuseAvatarState::Listening => ("Đang lắng nghe âm thanh (Listening)", 0.45),
            MuseAvatarState::Thinking => ("Đang suy luận ReAct (Thinking)", 0.25),
            MuseAvatarState::Speaking => ("Đang phản hồi giọng nói (Speaking)", 0.85),
            MuseAvatarState::Executing => ("Đang thực thi Sandbox (Executing)", 0.60),
        };

        let ascii_portrait = format!(
            "   .───────────.\n\
             .─'  .:ΩΩΩΩ:.   '─.\n\
            │   ┌───────────┐   │\n\
            │   │{eyes}│   │\n\
            │   │     ╷     │   │\n\
            │   │{mouth}│   │\n\
            │   └───────────┘   │\n\
             '─.  [Ω OMEGA]  .─'\n\
                '───────────'"
        );

        MuseAvatarFrame {
            state: self.state,
            viseme,
            blinking,
            voice_energy_db: energy,
            status_label_vi: status_label_vi.to_string(),
            ascii_portrait,
        }
    }
}
