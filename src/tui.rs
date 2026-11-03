//! # Giao diện Terminal TUI Đa Chủ đề & Web Dashboard (`src/tui.rs`)
//!
//! Hỗ trợ 4 chủ đề màu sắc (Themes):
//! - `Obsidian` (Mặc định)
//! - `RustTitanium`
//! - `CyberMatrix`
//! - `SolarLight`
//! Cùng bảng điều khiển TUI chia khung hỗ trợ đầy đủ Tiếng Việt chuẩn (`Locale::Vi`).

use crate::agent::HermesUnifiedAgent;
use crate::i18n::{I18n, Locale};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTheme {
    Obsidian,
    RustTitanium,
    CyberMatrix,
    SolarLight,
}

impl UiTheme {
    pub fn next(&self) -> Self {
        match self {
            UiTheme::Obsidian => UiTheme::RustTitanium,
            UiTheme::RustTitanium => UiTheme::CyberMatrix,
            UiTheme::CyberMatrix => UiTheme::SolarLight,
            UiTheme::SolarLight => UiTheme::Obsidian,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            UiTheme::Obsidian => "Omega Obsidian",
            UiTheme::RustTitanium => "Rust Titanium",
            UiTheme::CyberMatrix => "Cyber Matrix",
            UiTheme::SolarLight => "Solar Light",
        }
    }

    pub fn ansi_border_color(&self) -> &'static str {
        match self {
            UiTheme::Obsidian => "\x1b[38;5;208m",     // Rust Orange
            UiTheme::RustTitanium => "\x1b[38;5;214m", // Amber Gold
            UiTheme::CyberMatrix => "\x1b[38;5;46m",   // Matrix Green
            UiTheme::SolarLight => "\x1b[38;5;33m",    // Azure Blue
        }
    }
}

pub struct MultiThemeTui {
    pub theme: UiTheme,
    pub locale: Locale,
}

impl Default for MultiThemeTui {
    fn default() -> Self {
        Self {
            theme: UiTheme::Obsidian,
            locale: Locale::Vi,
        }
    }
}

impl MultiThemeTui {
    pub fn new(theme: UiTheme, locale: Locale) -> Self {
        Self { theme, locale }
    }

    /// Kết xuất khung giao diện TUI đa cột ra Terminal với màu sắc theo chủ đề
    pub fn render_dashboard_frame(&self, agent: &HermesUnifiedAgent, active_agent: &str) -> String {
        let c = self.theme.ansi_border_color();
        let reset = "\x1b[0m";
        let bold = "\x1b[1m";
        let fleet = agent.harness.list_fleet();
        let skills = agent.skill_manager.list_skills().unwrap_or_default();

        let fleet_lines = if fleet.is_empty() {
            "  • (Chưa có Agent nào đang chạy)".to_string()
        } else {
            fleet
                .iter()
                .take(5)
                .map(|s| format!("  • {} [{:?}] — {}", s.agent_name, s.status, s.inferred_role))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let skill_lines = if skills.is_empty() {
            "  • (Sẵn sàng tự tổng hợp SKILL.md từ hội thoại)".to_string()
        } else {
            skills
                .iter()
                .take(5)
                .map(|s| format!("  • {}: {}", s.name, s.description))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "{c}╔══════════════════════════════════════════════════════════════════════════════╗{reset}\n\
             {c}║{reset} {bold}{}{reset}\n\
             {c}║{reset} Giao diện (Theme): {bold}{}{reset} | Ngôn ngữ: {bold}{}{reset} | Agent: {bold}{active_agent}{reset}\n\
             {c}╠══════════════════════════════════════╦═══════════════════════════════════════╣{reset}\n\
             {c}║{reset} {bold}[ĐỘI HÌNH OPENHUMAN FLEET ({}/512)]{reset}   {c}║{reset} {bold}[KỸ NĂNG TỰ HỌC (SKILL.MD: {})]{reset}\n\
             {}\n\
             {c}╠══════════════════════════════════════╩═══════════════════════════════════════╣{reset}\n\
             {}\n\
             {c}╠══════════════════════════════════════════════════════════════════════════════╣{reset}\n\
             {c}║{reset} {}\n\
             {c}║{reset} {}\n\
             {c}╚══════════════════════════════════════════════════════════════════════════════╝{reset}",
            I18n::t(self.locale, "app_banner"),
            self.theme.name(),
            self.locale.display_name(),
            fleet.len(),
            skills.len(),
            fleet_lines,
            skill_lines,
            I18n::t(self.locale, "transfer_ready"),
            I18n::t(self.locale, "tui_help"),
        )
    }
}
