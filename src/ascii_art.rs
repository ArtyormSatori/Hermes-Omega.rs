//! # Bộ kết xuất Đồ hoạ ASCII Đơn sắc (`src/ascii_art.rs`)
//!
//! Thiết kế theo phong cách kỹ thuật tối giản, không loè loẹt (Monochrome Brutalist Engineering):
//! 1. Biểu tượng ASCII `Ω HERMES OMEGA.RS` chuẩn tỷ lệ monospace cho TUI và CLI.
//! 2. Bộ chuyển đổi ma trận độ sáng (`luminance_to_ascii`) biến ảnh hoặc khung hình Avatar thành ký tự ASCII.

/// Biểu tượng ASCII đơn sắc chính thức của Hermes Omega.rs
pub const HERMES_OMEGA_ASCII_EMBLEM: &str = r#"
  \......\//////////      _--:ΩΩ++ΩΩ:-+_      \\\\\\\\\\....../
   \.......\////////:-  /###/        \###\  -:\\\\\\\\......./
    \_______\//////.  /###/            \###\  .\\\\\\_______/
      \_.....\////:  |###|              |###|  :\\\\\....._/
        \______\//:  |###|              |###|  :\\\______/
          \_-...\:   \###\              /###/   :/...-_/
            \__===+*. \###\_          _/###/ .*+===__/
                    ::  '-##\        /##-'  ::
                    |Ω\____\#|      |#/____/Ω|
                    |Ω#######|      |#######Ω|
           [ Ω  H E R M E S   O M E G A . R S  —  v0.6.0 ]
"#;

/// Bảng ký tự độ sáng chuẩn từ tối đến sáng (10 mức)
const ASCII_RAMP: &[u8] = b" .:-=+*#%@";

/// Chuyển đổi mảng độ sáng (0..=255) kích thước `width x height` thành chuỗi nghệ thuật ASCII đơn sắc
pub fn luminance_grid_to_ascii(luminance: &[u8], width: usize, height: usize) -> String {
    let mut out = String::with_capacity((width + 1) * height);
    for row in 0..height {
        for col in 0..width {
            let idx = row * width + col;
            let lum = luminance.get(idx).copied().unwrap_or(0) as usize;
            let char_idx = (lum * (ASCII_RAMP.len() - 1)) / 255;
            out.push(ASCII_RAMP[char_idx] as char);
        }
        if row + 1 < height {
            out.push('\n');
        }
    }
    out
}
