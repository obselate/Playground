# Native Windows Markdown Notepad — Build Spec

> **Handoff document.** This is the complete, self-contained specification for a new project. A fresh Cursor agent should be able to start from this file alone, with no prior conversation context, and build the product to v1.

---

## 1. Product identity

- **One-liner:** A sub-2 MB, blazing-fast, native Windows markdown notepad with sticky notes.
- **Name:** *TBD.* Shortlist: `Noted`, `Scrib`, `Jot`, `Quill`, `Parch`, `Memo`, `Stik`. Pick one before first release; placeholder in code is `notepad` / `NotepadApp`.
- **Positioning:** The app Notepad should have been in 2026 — minimal, native, fast, gorgeous markdown, with first-class sticky notes. Not VS Code, not Obsidian, not Typora.

### Non-goals (explicit)

- Not a full IDE or extension host.
- No plugins / scripting in v1.
- No cloud sync in v1 (local-first only).
- No cross-platform build in v1 (Windows 10 1809+ and Windows 11 only).
- No tabbed document UI in v1 — one document per window; stickies handle multi-doc.
- No math rendering (KaTeX), no Mermaid, no custom containers in v1.

---

## 2. Platform, constraints, budgets

| Constraint | Target |
|---|---|
| OS support | Windows 10 1809+, Windows 11 (x64, ARM64) |
| Binary size | **≤ 2 MB** stripped, single `.exe`, no installer required |
| Cold start | **≤ 100 ms** to first interactive frame |
| Runtime deps | **None** — static CRT, no .NET, no WebView2, no VC++ redist |
| Idle memory (1 doc) | ≤ 20 MB working set |
| Smooth-edit file size | Up to 50 MB without jank; 500 MB without crashing |
| Typing latency | < 16 ms at p99 on a 2019-era laptop |
| Theme switch | < 50 ms end-to-end, no flicker |
| DPI change | < 100 ms re-layout and re-render, crisp text |

---

## 3. Tech stack (locked)

| Layer | Choice |
|---|---|
| Language | Rust stable, `edition = "2021"` |
| Win32 / Direct2D / DirectWrite | [`windows`](https://crates.io/crates/windows) crate (official Microsoft) |
| Markdown parser | [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark) (CommonMark 0.30 + GFM) |
| Text buffer | [`ropey`](https://crates.io/crates/ropey) |
| Syntax highlighting | [`syntect`](https://crates.io/crates/syntect) with a trimmed bundled syntax/theme set |
| Config (de)serialization | `serde` + `toml` |
| UUIDs | `uuid` |
| Logging (debug only; stripped in release) | `tracing` |
| 2D rendering | `ID2D1Factory1` + DXGI swap chain + `ID2D1DeviceContext` |
| Text shaping | `IDWriteFactory` + cached `IDWriteTextFormat` per (font, size, weight) |

### Build profile (required in `Cargo.toml`)

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

- Link with `+crt-static` to eliminate the VC++ runtime dependency.
- Ship a Windows app manifest declaring **per-monitor V2 DPI awareness** and Common Controls 6.
- Target triple for primary release: `x86_64-pc-windows-msvc`. Also produce `aarch64-pc-windows-msvc`.

### Explicitly rejected alternatives (and why)

- **Electron / Tauri** — wrong universe for "minimal." Electron is 80–150 MB; Tauri still pulls WebView2 and a web stack.
- **C# / WPF / WinUI 3** — ~10–15 MB with NativeAOT, slower cold start, doesn't feel "no BS."
- **Qt (C++)** — heavy (~20–40 MB), overkill.
- **Flutter Windows / Fyne / Gio / Slint** — paint their own widgets, so they're never *truly* native; Slint also busts the ≤ 2 MB budget (realistic ~4–7 MB for this app).
- **Go** — 5–10× larger binaries than Rust/C++; no maintained first-class Win32 widget toolkit.
- **C++ + Win32** — would give smallest binary (~100–500 KB) but loses Rust's safety and ergonomics. Rust is the chosen balance.

---

## 4. UI customization strategy (locked)

This is the architectural backbone. Every UI decision derives from these six rules.

1. **Custom-render** the editor, markdown preview, and sticky-note content surfaces via Direct2D + DirectWrite. We own paint, hit-testing, caret, selection, scroll, and animation on these.
2. **Stock Win32** for menus, settings dialog, find/replace, file dialogs, font picker, system tray menu. Fast to build, accessible, feels right.
3. **Custom title bar** on the main window and stickies via `DwmExtendFrameIntoClientArea` + `WM_NCCALCSIZE` + `WM_NCHITTEST`, with correct snap-layout hotspot handling (`HTMAXBUTTON` on the maximize button).
4. **Dark mode** follows system; applied via `SetWindowTheme(hwnd, "DarkMode_Explorer", NULL)` + `DwmSetWindowAttribute(..., DWMWA_USE_IMMERSIVE_DARK_MODE, ...)`. Custom surfaces own their palette and react to `WM_SETTINGCHANGE` ("ImmersiveColorSet") for live theme switches.
5. **Per-monitor V2 DPI** awareness declared in the app manifest from M1. Handle `WM_DPICHANGED` on every window; custom surfaces read DPI on paint.
6. **Internal layout helper** (~100 lines) in `platform/layout.rs` providing dock / stack / grid primitives, applied in `WM_SIZE`. Avoids hand-positioning every control.

### Tier assignment per surface

| Surface | Tier | Notes |
|---|---|---|
| Main editor area | Custom (Tier 3) | DirectWrite styled runs, custom caret, selection, gutter |
| Markdown preview pane | Custom (Tier 3) | Typography, spacing, code-block theming, images, tables |
| Sticky notes content | Custom (Tier 3) | Custom title bar, tint, translucency, collapse animation |
| Main window chrome | Hybrid | DWM-extended custom title bar |
| Menu bar | Stock (Tier 1) | `SetWindowTheme` for dark mode |
| Status bar | Stock (Tier 1) | Owner-draw only if we need tint/icons later |
| Find/Replace bar | Custom panel (Tier 3) | Modeless, docked at top of editor |
| Settings dialog | Stock (Tier 1) | Modal, native controls |
| System tray icon & menu | Stock (Tier 1) | `Shell_NotifyIcon`, `TrackPopupMenu` |

### Design defaults (locked unless noted)

- **Render target:** DXGI swap chain + `ID2D1DeviceContext` (smoother resize, future-proof for D3D effects).
- **Window backdrop:** Mica on Win11, opaque fallback on Win10, probed at runtime via DWM attribute support.
- **Custom title bar:** applied to both main window and stickies, shared code in `platform/chrome.rs`.
- **Snap-layout:** on by default on Win11; `HTMAXBUTTON` wired up for the hover flyout.
- **Caption button glyphs:** Segoe Fluent Icons on Win11; custom minimal glyph paths as fallback.
- **Settings:** modal. **Find/Replace:** modeless, docked at top of editor.

---

## 5. Core editor features (must-haves)

- [ ] Open, save, save-as, new, recent files (last 10)
- [ ] Drag-and-drop file to open
- [ ] UTF-8 default; detect and preserve UTF-8 BOM, UTF-16 LE/BE, Windows-1252
- [ ] Line-ending detection + preservation (CRLF / LF / CR); conversion command
- [ ] Word wrap toggle (per window, remembered)
- [ ] Font family + size picker via native `ChooseFont`
- [ ] Line numbers toggle
- [ ] Find / Find next / Replace / Replace all (case-sensitive, whole word, regex)
- [ ] Go to line
- [ ] Undo / redo (unlimited in session, grouped by word-boundary)
- [ ] Cut / copy / paste / select all / duplicate line / delete line
- [ ] Auto-indent; tab width configurable; tabs vs. spaces toggle
- [ ] Zoom in/out (Ctrl+scroll, Ctrl+= / Ctrl+-)
- [ ] Status bar: line/col, selection length, encoding, line endings, word count
- [ ] Autosave draft on crash, recoverable on next launch
- [ ] File-change-on-disk detection with reload prompt (`ReadDirectoryChangesW`)

---

## 6. Markdown rendering

- **Parser:** `pulldown-cmark` (CommonMark 0.30 + GFM tables, task lists, strikethrough).
- **Renderer:** DirectWrite text layout with styled runs. **No HTML, no WebView.**
- **Modes:** Edit only / Preview only / Side-by-side / WYSIWYG-lite (inline styled editing).
- **Syntax highlighting:** `syntect` in fenced code blocks, bundled minimal theme/syntax set (trimmed to stay within binary budget — exact set tuned during M6).
- **Scroll sync:** between editor and preview pane.
- **Images:** relative PNG/JPG/WebP via Windows Imaging Component (WIC).
- **Links:** Ctrl+click opens in default browser.
- **Copy-as-HTML:** to clipboard (for pasting into email).

**v1 supported elements:** headings, bold, italic, strikethrough, inline/fenced code, blockquote, lists, task lists, tables, horizontal rule, links, images, footnotes.
**v1 explicitly deferred:** math (KaTeX), Mermaid, custom containers.

---

## 7. Sticky notes

The feature that differentiates this app from every other markdown editor on Windows.

- **Window model:** Each sticky is a top-level `HWND` with `WS_EX_TOPMOST` and a thin custom title bar. They are real Windows windows, not child widgets.
- **Lifecycle:**
  - Convert current document to sticky (menu + hotkey).
  - "New sticky" spawns an empty sticky at cursor / last-used size.
  - Optional **global hotkey** (e.g. `Ctrl+Alt+N`) creates a new sticky from anywhere. On by default.
- **Persistence:** Each sticky stored as a Markdown file plus a TOML sidecar with metadata.
  - Content: `%APPDATA%\<AppName>\stickies\<uuid>.md`
  - Metadata: `%APPDATA%\<AppName>\stickies\<uuid>.toml` (position, size, font, word-wrap, pinned, tint, collapsed state)
- **Appearance:**
  - 6–8 preset accent colors.
  - Optional translucent Mica/Acrylic backdrop on Win11.
  - Win11 rounded corners via DWM.
- **Interaction:**
  - Double-click title bar collapses to title only.
  - System tray "Hide all stickies" / "Show all stickies" toggle.
  - Stickies survive reboot; restored to last positions (clamped to currently-visible monitors).

---

## 8. UI / UX principles

- Native Win32 menus, native dialogs, native window chrome behavior (drag, resize, snap, Win11 rounded corners via DWM).
- No ribbons, no sidebars by default, no welcome screen, no telemetry, no "sign in."
- **Keyboard-first:** every command has a shortcut; shortcuts are discoverable via menu.
- Dark mode follows system; live-switches without flicker.
- High-DPI aware (per-monitor V2); scales cleanly from 100% to 400%.
- **Accessibility:** UIA (Microsoft UI Automation) providers on custom surfaces (editor, preview, sticky content). Stock controls get UIA for free.

---

## 9. Settings

- TOML file at `%APPDATA%\<AppName>\config.toml`.
- Settings window is a simple native modal dialog, not a whole sub-app.
- Configurable:
  - Default font family, size
  - Default encoding
  - Tab width, tabs vs. spaces
  - Autosave interval
  - Theme mode (system / light / dark)
  - Markdown rendering mode (edit only / preview only / side-by-side / WYSIWYG-lite)
  - Global hotkey binding (for new sticky)
  - Recent files count
  - Mica/Acrylic on/off

---

## 10. File & data layout

```
%APPDATA%\<AppName>\
├── config.toml          # global settings
├── recent.json          # recent files
├── crash-drafts\        # autosave snapshots
│   └── <uuid>.md
└── stickies\
    ├── <uuid>.md        # sticky content
    └── <uuid>.toml      # sticky metadata
```

---

## 11. Architecture

```
src/
├── main.rs               # entry, WinMain, message loop
├── app.rs                # App state, window registry, command dispatch
├── ui/
│   ├── main_window.rs    # editor window wndproc
│   ├── sticky.rs         # sticky wndproc
│   ├── menu.rs           # native menu construction
│   ├── dialogs.rs        # find/replace bar, settings dialog
│   └── tray.rs           # system tray icon + menu
├── editor/
│   ├── buffer.rs         # ropey-backed text model
│   ├── view.rs           # DirectWrite rendering + hit testing
│   ├── input.rs          # key/mouse -> edit ops
│   └── undo.rs           # undo stack
├── md/
│   ├── parse.rs          # pulldown-cmark driver
│   ├── layout.rs         # events -> DWrite styled runs
│   └── highlight.rs      # syntect integration
├── fs/
│   ├── io.rs             # encoding-aware read/write
│   ├── watch.rs          # ReadDirectoryChangesW
│   └── recent.rs
├── config.rs
└── platform/
    ├── chrome.rs         # custom title bar (WM_NCCALCSIZE, WM_NCHITTEST, caption buttons, snap hotspot)
    ├── theme.rs          # dark mode attach, WM_SETTINGCHANGE, palette tokens
    ├── dpi.rs            # per-monitor V2 helpers, WM_DPICHANGED glue
    ├── layout.rs         # dock/stack/grid layout helper (~100 lines)
    ├── dwrite.rs         # IDWriteFactory wrapper, text format cache, shaped-run cache
    ├── d2d.rs            # D2D factory, per-HWND render target, device-lost recovery
    └── hotkey.rs         # RegisterHotKey wrapper
```

### Module responsibility matrix

| Concern | Module |
|---|---|
| Title bar paint + hit test | `platform/chrome.rs` (shared by main + stickies) |
| Caption buttons (min/max/close) | `platform/chrome.rs` |
| Dark mode attach on window create | `platform/theme.rs` (called from every `WM_CREATE`) |
| Theme change propagation | `platform/theme.rs` |
| DPI change handling | `platform/dpi.rs` |
| Control positioning | `platform/layout.rs` |
| Text shaping cache | `platform/dwrite.rs` |
| Device-lost recovery | `platform/d2d.rs` |

---

## 12. Milestones (technical scope, not calendar)

### M1 — Skeleton
- Win32 window class, message loop.
- App manifest with per-monitor V2 DPI and Common Controls 6.
- Direct2D + DirectWrite hello-world paint.
- Dark mode attach on window create.
- Custom title bar with working min/max/close + snap-layout hotspot.
- Layout helper stub.
- **Exit criteria:** builds < 500 KB; launches in < 100 ms; snap-layout flyout works; live dark-mode switch works.

### M2 — Editor core
- ropey buffer, caret, selection.
- Keyboard input, IME basics.
- Clipboard.
- Undo stack grouped by word boundary.
- Word wrap toggle.

### M3 — File I/O
- Encoding-aware open/save.
- Line-ending detection & preservation.
- Recent files, drag-drop open, `ReadDirectoryChangesW` reload prompt.

### M4 — Menus & dialogs
- Native menu bar, keyboard shortcuts.
- Find / Replace modeless docked bar.
- Native `ChooseFont`.
- Status bar (line/col, encoding, line endings, word count).

### M5 — Markdown preview
- Split view.
- `pulldown-cmark` → DirectWrite styled runs.
- Scroll sync.
- Ctrl+click links; copy-as-HTML.

### M6 — Syntax highlighting
- `syntect` in fenced code blocks, bundled trimmed syntax/theme set.

### M7 — Stickies
- Sticky window class, persistence (MD + TOML sidecar).
- Tray icon, global hotkey.
- Collapse-to-title, preset tints, Mica/Acrylic on Win11.

### M8 — Polish
- Dark-mode flicker-free live switch.
- DPI across-monitor drag.
- Acrylic/Mica probing + fallback.
- Settings dialog.
- UIA providers on custom surfaces.
- Crash autosave + recovery.

### M9 — Release
- Signed `.exe` (code-signing cert).
- winget manifest.
- Optional MSIX for Microsoft Store.
- Both `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` builds.

---

## 13. Success criteria (v1 done-ness bar)

- Single `.exe` **≤ 2 MB** on disk.
- Runs on a clean Windows 10 1809 / Windows 11 VM with **no installs required**.
- Opens a 10 MB markdown file in **< 200 ms**.
- Typing latency **< 16 ms** at p99 on a 2019-era laptop.
- **Zero memory growth** over 24 h of idle editing (heap + GDI/USER objects).
- Every menu command reachable via keyboard.
- Theme change (light ↔ dark) completes in **< 50 ms**, no flicker.
- DPI change on monitor drag re-lays-out in **< 100 ms**, crisp text at new scale.
- Clean build: `cargo clippy -- -D warnings` and `cargo fmt --check` both pass.
- UIA: screen reader announces caret position and selection changes in the editor.

---

## 14. Open questions (defaults chosen; override before M1 if you disagree)

| # | Question | Default |
|---|---|---|
| 1 | App name | TBD — finalize before M9. Working name: `notepad`. |
| 2 | License | MIT. |
| 3 | ARM64 build at launch | Yes — `aarch64-pc-windows-msvc` shipped alongside x64. |
| 4 | Global hotkey for new sticky on by default | Yes, `Ctrl+Alt+N`, rebindable in settings. |
| 5 | Mica/Acrylic | Mica on Win11, opaque on Win10. Probed at runtime. |
| 6 | Syntax highlighting in v1 | Yes, with a trimmed `syntect` syntax/theme set. Re-evaluate if binary busts 2 MB. |
| 7 | Telemetry / crash reporting | Off. No telemetry, no network calls, ever. |
| 8 | Edit style | Ship **split-view** and **preview-only** in M5. Add **WYSIWYG-lite** in v1.5 if low-risk, else v2. |
| 9 | Distribution | Portable `.exe` (primary) + winget manifest. MSIX/Store optional post-v1. |
| 10 | Scope cuts to protect the 2 MB budget | In priority order if we bust budget: drop `syntect` → drop ARM64 from initial release → drop Mica → drop WIC images in preview. |

---

## 15. Reference apps (north stars)

- **Sublime Text** — the closest reference: tiny, fast, custom-rendered content, native chrome, no web runtime.
- **Notepad++** — for the "feels like a Windows app" baseline (but we aim higher visually).
- **Windows Terminal** — reference for custom title bar + DWM integration done right.
- **Typora** — the markdown UX bar to beat (but it's Electron — we should crush it on size and speed).
- **Windows 11 Sticky Notes** — reference for the sticky-note interaction model.

---

## 16. First commands a fresh Cursor agent should run

```bash
# 1. Scaffold
cargo new --bin notepad
cd notepad

# 2. Lock the release profile in Cargo.toml (see §3).

# 3. Add the dependencies:
cargo add windows --features "\
  Win32_Foundation,\
  Win32_Graphics_Direct2D,\
  Win32_Graphics_Direct2D_Common,\
  Win32_Graphics_DirectWrite,\
  Win32_Graphics_Dwm,\
  Win32_Graphics_Dxgi,\
  Win32_Graphics_Gdi,\
  Win32_Graphics_Imaging,\
  Win32_System_Com,\
  Win32_System_LibraryLoader,\
  Win32_UI_Controls,\
  Win32_UI_HiDpi,\
  Win32_UI_Input_KeyboardAndMouse,\
  Win32_UI_Shell,\
  Win32_UI_WindowsAndMessaging"
cargo add pulldown-cmark ropey syntect serde toml uuid tracing

# 4. Drop in the app manifest (per-monitor V2 DPI + Common Controls 6) via build.rs + embed-resource.

# 5. Start M1: window class, message loop, D2D hello-world paint.
```

The M1 skeleton must build under 500 KB and launch in under 100 ms before M2 begins. That's the architectural fitness test for the whole stack — if we can't hit it at M1, we won't hit ≤ 2 MB at v1.

---

*End of spec. Hand this file to a Cursor agent and point it at a fresh Windows dev environment. Begin at §16.*
