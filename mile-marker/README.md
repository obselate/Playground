# mile-marker

Record your screen with auto-pause-for-caption, and export the result as
a slideshow GIF or a Markdown walkthrough.

The premise: every time you write a tutorial, fix-it guide, or PR
description, you go through the same loop — take a screenshot, paste it
in, type what you were doing, repeat. `mile-marker` collapses the loop
into one workflow:

1. Press **Capture now** (or arm auto-capture every N seconds).
2. The app pauses and asks for a caption while the moment is fresh.
3. Press **Continue** (or click out of the caption box) to resume.
4. When you're done, **Export GIF** or **Export Markdown**.

The output is something you can paste straight into a README, a PR, or
chat — no second pass through Loom or screen-recorder editing.

## Install

Cross-platform native GUI built with [`eframe`/`egui`]:

```
cd mile-marker
cargo run --release
```

Or install the binary to your PATH:

```
cargo install --path .
mile-marker
```

Tested on Linux, macOS, and Windows. The release binary is a single file
with no external dependencies (the caption font is bundled).

### Linux dev requirements

eframe and xcap pull in X11/Wayland and PipeWire system libraries. On
Debian/Ubuntu:

```
sudo apt install libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
                 libxcb-xfixes0-dev libxkbcommon-dev libssl-dev \
                 libdbus-1-dev libegl-dev libgbm-dev libwayland-dev \
                 libpipewire-0.3-dev libspa-0.2-dev
```

(PipeWire and SPA come from xcap's Wayland screen-capture path; even on
X11 sessions they're required at build time.)

The GitHub Actions workflow installs these automatically.

## Usage

The window has three regions:

- **Toolbar**: project name, save / load, and the two **Export** buttons.
- **Capture controls**: a big blue **Capture now** button, an **auto**
  checkbox with an interval slider, a monitor picker, and a paused-state
  banner with **Continue**.
- **Frame strip**: a scrollable list of captures, each with a thumbnail,
  a caption box, capture time, source dimensions, and a delete button.

### Auto-capture with caption pause

When **auto** is enabled, `mile-marker` fires a capture every N seconds
(slider, default 8 s). Right after each capture it *stops the timer* and
waits for you to write a caption. The countdown only resumes when you
press **Continue** or click out of the caption box. The intent is that
you describe what you just did *while you remember*, then move on.

### Save / Load

A "session" is a directory containing:

```
my-session/
  session.json     # manifest: name, created_at, per-frame caption + timestamp
  frame-0000.png   # one PNG per frame, in capture order
  frame-0001.png
  ...
```

`session.json` is plain text, intentionally diff-friendly. The PNGs sit
next to it so you can poke at frames in any image viewer.

### Export

- **Export GIF**: writes `mile-marker.gif` next to the working directory.
  The exporter burns each caption onto a banner below the screenshot
  (so the screenshot is never obscured by text), pads frames to the
  largest dimensions, and uses a generous default frame delay (3 s)
  because the slideshow is meant for reading captions, not animating
  motion.
- **Export Markdown**: writes a `mile-marker-walkthrough/` directory
  with a `README.md` and a `frames/` subfolder. Captions are emitted
  verbatim so you can use markdown formatting (links, code, emphasis)
  inside them. The screenshot here is the **uncomposed** PNG — markdown
  is editable, so people will want to tweak the words after the fact.

## Layout

```
mile-marker/
  Cargo.toml
  assets/
    font.ttf            # bundled DejaVu Sans for caption overlays
    font.LICENSE        # font copyright
  src/
    lib.rs              # module declarations + EMBEDDED_FONT constant
    session.rs          # Session/Frame model + JSON persistence
    capture.rs          # cross-platform screenshot via xcap
    composer.rs         # caption-banner rendering
    export/
      mod.rs
      gif.rs
      markdown.rs
    app.rs              # egui App: state, controls, frame strip
    main.rs             # eframe entry point
```

The library is structured so the GUI is a thin layer over headless,
testable building blocks. The full unit/integration suite runs
without a display server (17 tests covering session round-trips,
caption wrapping, GIF/Markdown export shape).

## Caveats and known limits

- **The mile-marker window appears in its own captures.** A future fix
  is to hide it during capture; for now, drag it out of the way or use
  the monitor picker to capture a different screen.
- **No region select yet** — captures are always full-monitor.
- **GIF colour quantisation** uses the `gif` crate's neuquant. For UI
  screenshots this is fine; for photo-realistic content it's lossy. If
  you need higher fidelity, use the Markdown exporter (PNG frames).

## Development

```
cargo build
cargo test
cargo run
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

CI runs on Linux, macOS, and Windows; see
`.github/workflows/mile-marker.yml`.

## Releasing

Push a tag matching `mile-marker-v<semver>` to trigger
[`release-mile-marker.yml`](../.github/workflows/release-mile-marker.yml):

```
git tag mile-marker-v0.2.0
git push origin mile-marker-v0.2.0
```

That builds a stripped release binary for Linux x86_64, macOS x86_64
(Intel), macOS aarch64 (Apple Silicon), and Windows x86_64, bundles
each with the README, LICENSE, and the bundled-font copyright notice,
and uploads the archives to the GitHub Release for that tag.

## License

MIT for the code. The bundled DejaVu Sans font has its own permissive
license — see `assets/font.LICENSE`.
