# mmdr — Mermaid RS Renderer

<div align="center">
  <img src="mmdr_illustration.webp" alt="mmdr — 100–1400x faster Mermaid rendering in pure Rust">
</div>

<div align="center">

![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-blue.svg)
![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)
![License](https://img.shields.io/badge/License-MIT-yellow.svg)
![Crates.io](https://img.shields.io/crates/v/mermaid-rs-renderer.svg)
![Release](https://img.shields.io/github/v/release/quangdang46/mermaid-rs-renderer)

</div>

**100–1400× faster Mermaid rendering. Pure Rust. Zero browser dependencies.**  
Parse Mermaid natively and render straight to SVG (or PNG) — no Chromium, no Node, no Puppeteer.

<div align="center">

```bash
cargo install mermaid-rs-renderer
# or
echo 'flowchart LR; A-->B-->C' | mmdr -e svg
```

</div>

---

## 🤖 Agent Quickstart

```bash
# Pipe to stdout (fastest)
echo 'flowchart LR; A-->B-->C' | mmdr -e svg

# File to SVG (440× faster than mermaid-cli)
mmdr -i diagram.mmd -o output.svg -e svg

# With timing JSON
mmdr -i diagram.mmd -o out.svg -e svg --timing

# Library embed (Criterion: ~1.5 ms)
use mermaid_rs_renderer::render;
let svg = render("flowchart LR; A-->B-->C")?;
```

---

## TL;DR

### The Problem

Official `mermaid-cli` spawns **headless Chromium per diagram** — ~2s startup tax every time.

| Workload | mermaid-cli | Reality |
|----------|-------------|---------|
| 50 diagrams in CI | ~2 minutes | Browser tax dominates |
| Live editor preview | Multi-second lag | Unusable feedback loop |
| Batch docs | Coffee break | Throughput capped by Puppeteer |

### The Solution

**mmdr** parses Mermaid in Rust and emits SVG/PNG directly.

| Metric (typical) | mmdr | mermaid-cli | Speedup |
|------------------|------|-------------|---------|
| Flowchart | ~4.5 ms | ~2.0 s | **~440×** |
| Class | ~4.7 ms | ~1.9 s | **~410×** |
| State | ~4.0 ms | ~2.0 s | **~500×** |
| Sequence | ~2.7 ms | ~1.9 s | **~700×** |

Warm font cache: **500–900×**. Optional `--fastText`: **1600×+** on tiny diagrams.

### Why Use mmdr?

| Feature | What it does |
|---------|--------------|
| **Native parse + layout** | No browser process spawn |
| **SVG + PNG** | PNG via `resvg` (feature-gated) |
| **Library API** | Embed in Rust apps without CLI spawn |
| **Themes** | `default` · `dark` · `forest` · `neutral` · `modern` |
| **23 diagram types** | Flowchart, sequence, class, state, ER, gantt, … |
| **CI-friendly** | Single static binary, ~15 MB RAM vs ~300 MB Chromium |

---

### Quick Example

```bash
# Pipe to stdout
echo 'flowchart LR; A-->B-->C' | mmdr -e svg

# File → file
mmdr -i diagram.mmd -o output.svg -e svg

# Dark theme + timing JSON on stderr
mmdr -i diagram.mmd -o out.svg -e svg --theme dark --timing

# Fast text metrics (ASCII labels)
mmdr -i diagram.mmd -o out.svg -e svg --fastText

# PNG
mmdr -i diagram.mmd -o out.png -e png
```

---

## Design Philosophy

1. **Browser-free by default.**  
   If a doc tool needs Chromium just to draw boxes and arrows, something is wrong.

2. **Milliseconds matter in agent loops.**  
   Agents and live previews render diagrams constantly; 2s cold starts kill the loop.

3. **Library first, CLI second.**  
   Criterion-level in-process times (flowchart ~1.5 ms) beat even a fast CLI spawn.

4. **Visual parity is a ratchet, not a promise of pixel identity.**  
   Hard gates and conformance fixtures continuously reduce divergence from mermaid-js.

5. **Feature-gated weight.**  
   Drop `png` / `cli` features when embedding in servers or static-site generators.

---

## How mmdr Compares

| Use case | mermaid-cli | Kroki / remote | **mmdr** |
|----------|-------------|----------------|----------|
| CI with 50 diagrams | ~2 min | Network + queue | **&lt; 1 s** |
| Real-time preview | Lag | Latency | **Instant** |
| Embed in Rust apps | N/A | HTTP client | **Library API** |
| Runtime deps | Node + Chromium | Service | **Binary only** |
| Offline / air-gap | Heavy | ❌ | ✅ |
| Visual parity | Reference | Varies | Improving fast |

**When to use mmdr:**
- CI pipelines rendering many diagrams
- Local previews and agent toolchains
- Embedding Mermaid in Rust services / static generators

**When mmdr might not be ideal:**
- You need every experimental mermaid-js plugin / click handler
- Pixel-perfect identity with mermaid-cli is a hard release gate (diff goldens)

---

## Installation

```bash
# crates.io
cargo install mermaid-rs-renderer

# Homebrew
brew tap 1jehuang/mmdr && brew install mmdr

# Scoop (Windows)
scoop bucket add mmdr https://github.com/1jehuang/scoop-mmdr && scoop install mmdr

# AUR
yay -S mmdr-bin

# Nix flake
nix run github:1jehuang/mermaid-rs-renderer -- --help

# From this fork (source)
git clone https://github.com/quangdang46/mermaid-rs-renderer.git
cd mermaid-rs-renderer
cargo install --path .
```

> **Note:** Upstream package/Homebrew paths may track [`1jehuang/mermaid-rs-renderer`](https://github.com/1jehuang/mermaid-rs-renderer). This fork at `quangdang46/mermaid-rs-renderer` follows the same CLI (`mmdr`).

### Library-only (minimal deps)

```toml
[dependencies]
mermaid-rs-renderer = { version = "0.3", default-features = false }
```

| Feature | Default | Purpose |
|---------|---------|---------|
| `cli` | ✅ | `mmdr` binary |
| `png` | ✅ | PNG via resvg |

---

## Quick Start

```bash
# stdin → stdout
echo 'flowchart LR; A-->B-->C' | mmdr -e svg

# explicit stdin → file
echo 'flowchart LR; A-->B-->C' | mmdr -i - -o out.svg -e svg

# file → file
mmdr -i diagram.mmd -o output.svg -e svg

# size metadata only (JSON)
mmdr -i diagram.mmd --size

# layout dump for debugging
mmdr -i diagram.mmd -o out.svg --dumpLayout layout.json
```

### Library

```rust
use mermaid_rs_renderer::{render, render_with_options, RenderOptions};

let svg = render("flowchart LR; A-->B-->C")?;

let opts = RenderOptions::default();
let svg = render_with_options("sequenceDiagram\nAlice->>Bob: Hi", opts)?;
```

// Criterion-level raw render times (no process spawn):  
// flowchart ~1.5 ms · sequence ~0.07 ms

---

## Commands / CLI Reference

```text
mmdr [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-i, --input <PATH>` | Input `.mmd` file or `-` for stdin |
| `-o, --output <PATH>` | Output file (default: stdout for SVG) |
| `-e, --outputFormat <svg\|png>` | Output format (default: `svg`) |
| `-c, --configFile <PATH>` | Config JSON (Mermaid-like `themeVariables`) |
| `-t, --theme <NAME>` | `default` · `dark` · `forest` · `neutral` · `modern` |
| `-w, --width <N>` | Width (PNG fallback / sizing) |
| `-H, --height <N>` | Height (PNG fallback / sizing) |
| `--preferredAspectRatio <R>` | `width:height`, `width/height`, or decimal |
| `--nodeSpacing <N>` | Node spacing override |
| `--rankSpacing <N>` | Rank spacing override |
| `--dumpLayout <PATH>` | Dump computed layout JSON |
| `--timing` | Timing JSON on stderr |
| `--size` | Print size metadata JSON and exit |
| `--fastText` | Approximate text metrics (ASCII-heavy speed path) |

```bash
mmdr -i arch.mmd -o arch.svg -e svg --theme dark --timing
mmdr -i arch.mmd -o arch.png -e png -w 1200
mmdr -i arch.mmd --size
```

---

## Supported Diagram Types

| Type | Keyword |
|------|---------|
| Flowcharts | `flowchart` / `graph` (TD, TB, LR, RL, BT) |
| Sequence | `sequenceDiagram` |
| Class | `classDiagram` |
| State | `stateDiagram-v2` |
| ER | `erDiagram` |
| Pie | `pie` |
| XY chart | `xychart` |
| Quadrant | `quadrantChart` |
| Gantt | `gantt` |
| Timeline | `timeline` |
| Journey | `journey` |
| Mindmap | `mindmap` |
| Git graph | `gitGraph` |

Coverage continues to expand; verify your dialect against fixtures if you depend on edge syntax.

---

## Performance Notes

| Mode | When | Speedup vs mermaid-cli |
|------|------|------------------------|
| Cold | First run | 100–700× |
| Warm font cache | After first render | 500–900× |
| `--fastText` | Tiny ASCII-heavy diagrams | 1600–2000× |

Large diagrams (200 nodes) still land **~50–100×+** — layout cost grows, browser tax still dominates mermaid-cli.

Memory: ~**15 MB** vs ~**300 MB** for mermaid-cli.

---

## Architecture

```text
.mmd / stdin
    │
    ▼
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ Parser      │ ──▶ │ Layout       │ ──▶ │ Render      │
│ parser.rs   │     │ layout/*     │     │ render.rs   │
└─────────────┘     └──────────────┘     └──────┬──────┘
                                                │
                                    ┌───────────┼───────────┐
                                    ▼           ▼           ▼
                                  SVG         PNG*      layout dump
                                              (resvg)

* feature = "png"
```

| Module | Role |
|--------|------|
| `parser` | Mermaid → IR |
| `layout` | Placement, routing, subgraph containment |
| `render` | SVG emit (+ optional PNG) |
| `theme` / `config` | Themes + Mermaid-like variables |
| `text_metrics` | Font / `--fastText` widths |
| `cli` | `mmdr` binary |

---

## Troubleshooting

### `mmdr: command not found`

```bash
cargo install mermaid-rs-renderer
# ensure ~/.cargo/bin is on PATH
export PATH="$HOME/.cargo/bin:$PATH"
```

### PNG output fails / feature missing

Build with default features (includes `png`), or:

```bash
cargo install mermaid-rs-renderer --features png
```

### Labels look wrong with `--fastText`

`--fastText` uses approximate ASCII widths. For CJK / proportional fonts, omit the flag.

### Visual mismatch vs mermaid-cli

Expected for some edge cases. Diff golden SVGs/PNGs in CI if pixel identity matters:

```bash
mmdr -i fixture.mmd -o out.svg -e svg
diff -u golden.svg out.svg
```

### Parse errors on newer Mermaid syntax

File an issue with a minimal `.mmd` repro. Not every experimental mermaid-js extension is implemented yet.

---

## Limitations

### What mmdr Doesn't Do (Yet)

- **Not a full Mermaid JS runtime** — no browser plugins / live click handlers
- **Visual parity** is improving fast but not every edge case matches mermaid-cli
- **`--fastText`** is calibrated for ASCII — non-Latin labels may shift

### Known Limitations

| Capability | Current state | Notes |
|------------|---------------|-------|
| Diagram coverage | ✅ Broad | Verify your dialect |
| Pixel-perfect parity | ⚠️ Partial | Use golden diffs |
| Remote Kroki-style API | ❌ | Library/CLI only |
| JS plugin ecosystem | ❌ | Out of scope |

---

## FAQ

### Drop-in for mermaid-cli?

Same SVG *goal*, different pipeline. Diff golden images in CI if pixel-perfect parity matters.

### Why so much faster?

No Chromium process spawn. Native parse + layout + SVG emit.

### Embed in a server?

Yes — use as a Rust library to avoid CLI spawn entirely. Disable default features for a thinner dependency graph.

### Themes?

```bash
mmdr -i d.mmd -o d.svg --theme dark
mmdr -i d.mmd -o d.svg --theme forest
```

Custom: pass `--configFile` with Mermaid-like `themeVariables`.

### Upstream vs this fork?

Packaging/Homebrew often tracks `1jehuang/mermaid-rs-renderer`. This repository is `quangdang46/mermaid-rs-renderer` with the same `mmdr` CLI surface.

---

## About Contributions

Please don't take this the wrong way, but I do not accept outside contributions for any of my projects. I simply don't have the mental bandwidth to review anything, and it's my name on the thing, so I'm responsible for any problems it causes; thus, the risk-reward is highly asymmetric from my perspective. I'd also have to worry about other "stakeholders," which seems unwise for tools I mostly make for myself for free. Feel free to submit issues, and even PRs if you want to illustrate a proposed fix, but know I won't merge them directly. Instead, I'll have Claude or Codex review submissions via `gh` and independently decide whether and how to address them. Bug reports in particular are welcome. Sorry if this offends, but I want to avoid wasted time and hurt feelings. I understand this isn't in sync with the prevailing open-source ethos that seeks community contributions, but it's the only way I can move at this velocity and keep my sanity.

---

## License

[MIT](LICENSE)

---

<div align="center">

**Native Mermaid. Browser-free. Agent-friendly.**

</div>
