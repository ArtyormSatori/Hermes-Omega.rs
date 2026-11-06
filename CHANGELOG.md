# Changelog

All notable changes to Hermes Omega.rs, per release.
Mọi thay đổi đáng chú ý của Hermes Omega.rs, theo từng phiên bản.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/).

---

## [1.0.1] — 2026-11-06 — Patch Release / Bản vá

### Fixed / Sửa

- `security::sanitize_fts5_query`: FTS5 operator keywords glued to arguments (e.g. `NEAR(a,b)`) no longer survive sanitization as `NEARab` — the keyword is now stripped and only the data (`ab`) is kept. Fixes the failing CI test `test_sql_and_fts5_injection_guard` (CI was red on every run since v0.5.0).
  `security::sanitize_fts5_query`: từ khoá toán tử FTS5 dính với đối số (vd `NEAR(a,b)`) không còn lọt qua bộ lọc thành `NEARab` — từ khoá bị gỡ, chỉ giữ phần dữ liệu (`ab`). Sửa test CI `test_sql_and_fts5_injection_guard` vốn đỏ trên mọi lượt chạy từ v0.5.0.
- `ascii_art`: emblem version corrected `v0.6.0` → `v1.0.0`.
  `ascii_art`: sửa version trong emblem từ `v0.6.0` → `v1.0.0`.
- README: Vietnamese proofread (fixed mistranslations: `căn lề prefix-cache`, `vòng học kín`, `khiên bảo mật`, `HMAC-SHA256 hằng số thời gian`); fleet limits explained in plain language; Screenshots section added (ASCII banner/emblem, app icon).
  README: soát lại tiếng Việt (sửa các chỗ dịch sai); giải thích giới hạn hạm đội bằng ngôn ngữ thường; thêm mục hình ảnh demo (banner/emblem ASCII, icon ứng dụng).

---

## [1.0.0] — 2026-11-03 — Final Release / Bản phát hành chính thức

### Added / Thêm mới

- CLI with 10 modes: `tui`, `chat`, `daemon`, `security-audit`, `fleet`, `teach`, `skills`, `search`, `acp`, `doctor`.
  CLI 10 chế độ: `tui`, `chat`, `daemon`, `security-audit`, `fleet`, `teach`, `skills`, `search`, `acp`, `doctor`.
- MUSE.AI realtime Vietnamese avatar (`avatar`): viseme lip-sync, `Thinking` / `Executing` / `Speaking` states, wired into the agent loop and the gateway (`/v1/avatar/frame`).
  Avatar MUSE.AI tiếng Việt thời gian thực: khẩu hình viseme, trạng thái `Thinking` / `Executing` / `Speaking`, nối vào vòng lặp agent và gateway (`/v1/avatar/frame`).
- Multi-theme TUI (`tui`) with ASCII emblem and Vietnamese localization (`i18n`, `Locale::Vi`).
  TUI đa chủ đề với emblem ASCII và bản địa hoá tiếng Việt.
- >1 GB–16 GB streaming (`claw_gateway`, `large_transfer`): chunked upload (`/v1/files/chunk`), `Range` download (HTTP 206), `DefaultBodyLimit::disable()`.
  Truyền tải >1 GB–16 GB: upload theo chunk, download theo `Range` (HTTP 206), gỡ giới hạn 2 MB của Axum.
- Web dashboard served at `GET /` and `GET /dashboard` on the Axum gateway (port 8080).
  Dashboard web tại `GET /` và `GET /dashboard` trên gateway Axum (cổng 8080).
- Branding assets under `assets/`: banner, icon, ASCII banner/emblem, SVG icon.
  Hình ảnh thương hiệu trong `assets/`: banner, icon, banner/emblem ASCII, icon SVG.

### Changed / Thay đổi

- Binary renamed `hermes` → `hermes-omega`; default agent `hermes-main` → `hermes-omega`.
  Binary đổi tên `hermes` → `hermes-omega`; agent mặc định đổi thành `hermes-omega`.
- `claw_gateway`: DM pairing now single master code verified with `constant_time_eq`; sliding-window `RateLimiter` on the gateway.
  `claw_gateway`: ghép đôi DM dùng một mã chính, xác thực bằng `constant_time_eq`; giới hạn tần suất theo cửa sổ trượt.
- `sandbox`: cached ANSI regex (`OnceLock`), shell-command audit, child env isolation (`env_clear` + allow-list `PATH`, `LANG=vi_VN.UTF-8`).
  `sandbox`: cache regex ANSI, kiểm duyệt lệnh shell, cô lập biến môi trường tiến trình con.
- `agent`: every turn scanned by `PromptInjectionGuard::scan_and_spotlight` and checked by `verify_no_canary_leak`.
  `agent`: mỗi lượt đều quét bằng `PromptInjectionGuard::scan_and_spotlight` và kiểm tra rò rỉ canary.

### Fixed / Sửa

- `hermes_core` v0.8.0 compat items (`TrajectoryExporter`, `HermesCronJob`, `update_honcho_peer`, `update_memory_file`) restored so `main.rs` compiles.
  Khôi phục các mục tương thích của `hermes_core` v0.8.0 để `main.rs` biên dịch được.

---

## [0.8.0-sec] — 2026-11-03 — Security Hardening / Bản gia cố bảo mật

### Added / Thêm mới

- 6-layer defense-in-depth security (`security` v0.8.0, `large_transfer`).
  Khiên bảo mật 6 lớp theo chiều sâu.
- `PromptInjectionGuard`: canary token in Tier 1 of the system prompt, cryptographic spotlighting (`<|UNTRUSTED_DATA_{nonce}|>`), Unicode stripper.
  `PromptInjectionGuard`: canary token ở tầng 1 của system prompt, spotlighting mật mã, lọc Unicode.
- `SqlSecurityGuard`: FTS5 query sanitizer (blocks `NEAR`, `*`, `:`, `UNION SELECT`), SQL input audit.
  `SqlSecurityGuard`: bộ lọc truy vấn FTS5, kiểm duyệt đầu vào SQL.
- SQLite hardening in `hermes_core`: `PRAGMA trusted_schema = OFF`, `busy_timeout(5s)`, WAL journal, `unicode61 remove_diacritics 2` tokenizer.
  Gia cố SQLite: `trusted_schema = OFF`, `busy_timeout(5s)`, WAL, tokenizer `unicode61 remove_diacritics 2`.
- New modules: `ascii_art`, `avatar`, `tui`, `large_transfer`, `i18n`.
  Module mới: `ascii_art`, `avatar`, `tui`, `large_transfer`, `i18n`.

---

## [0.5.0] — 2026-11-03 — Initial Release / Bản phát hành đầu tiên

### Added / Thêm mới

- Unified Rust agent runtime (Edition 2021): Hermes 5-layer FTS5 memory & closed skill loop, OpenHands event stream & context condenser, OpenClaw multi-channel gateway & heartbeat, OpenHuman lock-free fleet harness & memory trees, Grokbot role inference & task recording.
  Runtime agent hợp nhất (Edition 2021): bộ nhớ FTS5 5 lớp & vòng học kín của Hermes, event stream & context condenser của OpenHands, gateway đa kênh & heartbeat của OpenClaw, hạm đội lock-free & cây tri thức của OpenHuman, suy luận vai trò & ghi task của Grokbot.
- 19 source modules, Docker + docker-compose, systemd service, CI workflow (check, clippy, test, release build).
  19 module nguồn, Docker + docker-compose, systemd service, CI (check, clippy, test, build release).

[1.0.1]: https://github.com/ArtyormSatori/Hermes-Omega.rs/releases/tag/v1.0.1
[1.0.0]: https://github.com/ArtyormSatori/Hermes-Omega.rs/releases/tag/v1.0.0
[0.8.0-sec]: https://github.com/ArtyormSatori/Hermes-Omega.rs/releases/tag/v0.8.0-sec
[0.5.0]: https://github.com/ArtyormSatori/Hermes-Omega.rs/releases/tag/v0.5.0
