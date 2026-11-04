# Design review: native desktop feel

_Status: open · 2026-11-04 · reviewed after M2-1 (app shell) on macOS in dark mode._

Goal: ScribbleCap should feel like a modern native app on macOS, Windows and Linux,
not a website in a window. This review lists what to change and the direction to
change it in. Tasks it produces are linked from [roadmap.md](roadmap.md).

## Findings

Sources: `ui/src/App.svelte`, `ui/src/style.css`, `app/tauri.conf.json`, and macOS
screenshots (empty window, image open, narrow window; system appearance Dark).

| # | Problem | Native expectation |
|---|---|---|
| 1 | **Ignores dark mode.** With macOS in Dark, the toolbar, footer and empty state are all light. The native title bar is light too, so the window theme is forced light somewhere (config or webview background). | Follow the system light/dark setting in both the native window and the CSS. |
| 2 | **Two stacked bars:** system title bar plus a separate toolbar, about 85px before content. | macOS: overlay title bar with tools beside the window buttons. Windows: tools in the title bar, window controls on the right. Linux: GNOME-style header bar. |
| 3 | **Logo and app name in the toolbar.** | The window title and Dock/taskbar icon identify the app; use the space for tools. |
| 4 | **Pink "Open image" button in the toolbar** and a second pink button in the empty state; they are the loudest things on screen. | Quiet icon buttons in toolbars; open via File menu, ⌘/Ctrl O and one button in the empty state. |
| 5 | **Pink used for focus and app controls.** | System accent colour for focus, the selected tool and selection handles. Pink is for annotations only. |
| 6 | **No menu bar.** | macOS File/Edit/View/Window menus; at least a File menu on Windows and Linux (Tauri menus). |
| 7 | **Footer strip with ~10px text** ("Your canvas, ready when you are", help hints). | Remove. Show size and zoom as a small badge on the canvas; shortcuts go in tooltips and menus. |
| 8 | **Shortcut labels say "Ctrl/⌘".** | Show the current platform's key only: ⌘O on macOS, Ctrl+O elsewhere. |
| 9 | **Dropping a file on the window isn't supported.** | Drag a file in to open it. The empty-state corner-bracket art can become the drop-target outline. |
| 10 | **Canvas switches from light grey to dark slate** when an image opens. | One canvas colour per theme, always darker than the window chrome. |
| 11 | **Image edge disappears** on the dark canvas: a dark screenshot on `#2e3340` has no visible boundary. | A 1px edge or soft shadow around the image. |
| 12 | **Resizing doesn't keep the image fitted;** a narrow window cuts off the image's left edge until Fit is pressed again. | Stay fitted while resizing until the user zooms manually (as Preview and Photos do). Changes the M2-1 rule in [app-shell.md](app-shell.md). |
| 13 | **Grab cursor over the whole canvas.** | Once drawing tools exist (M2-2), drag draws; pan with space+drag, scroll or middle button. |
| 14 | **Small, web-styled type:** 10–12px, negative letter-spacing, tiny tracked "PNG · JPEG · …" line. | Native sizes: ~13px macOS, 14px Windows 11, 14–15px GNOME. No extra letter-spacing. Write "Opens PNG, JPEG, WebP, TIFF, BMP and GIF." |

Also: the checkerboard tile is rebuilt on every canvas redraw; cache it before tools
start redrawing on every pointer move.

Working as intended: the window title shows the file name; the toolbar stays on one
line at narrow widths; the empty-state artwork.

## Direction

The window frame should disappear; the user's image and a few bold annotation
colours are the only loud things on screen.

### Colours

Prefer CSS system colours (`AccentColor`, `Canvas`, `CanvasText`) and `light-dark()`.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--chrome` | `#ECECEE` | `#2A2A2D` | Title bar and toolbar |
| `--stage` | `#D9DADD` | `#1C1C1E` | Area behind the image; always darker than chrome |
| `--ink` | `#1D1D1F` | `#ECECEE` | Interface text |
| `--ink-2` | `#6E6E73` | `#98989D` | Secondary text, inactive icons |
| `--accent` | `AccentColor` (fallback `#0A64D8`) | same | Focus, selected tool, selection handles |
| `--mark` | `#E5194A` | `#FF3A68` | Default annotation colour; the only brand colour |

### Type

System font per OS: SF Pro (`-apple-system`) on macOS, Segoe UI Variable on Windows,
Cantarell / Ubuntu / Noto Sans on Linux. Two sizes: body 13px (macOS) / 14px
(Windows, Linux); empty-state heading 20px semibold. No extra letter-spacing.

### Layout

Tools join the title bar; window controls stay where each OS puts them; no footer.

```
macOS
┌─●●●──[▸ ↗ ╱ ▭ ◯ T ✎ ▒ ①]──[■ ━]────────────[− 100% +]──[⇪]─┐
│                                                               │
│                    ┌──────────────────┐                       │
│                    │      image       │                       │
│                    └──────────────────┘          ┌─────────┐  │
│                                                  │1920×1080│  │
└──────────────────────────────────────────────────┴─────────┴──┘

Windows / Linux: same row with window controls on the right ( – ▢ ✕ );
Linux follows the theme's button layout.
```

- Left: tools (select, arrow, line, rectangle, ellipse, text, marker, pixelate, step
  number), each on one key.
- Middle: current annotation colour as one large swatch, plus stroke size.
- Right: zoom, then share (copy, save, drag-out).
- Groups are left-aligned; the canvas is centred.

### The distinctive element: drag-out

Dragging the finished image straight into a chat or email is the core of the
product. Make that one control distinctive: a small tab with a thumbnail of the
annotated image, pinned to the image corner, that you drag out of the window.
Everything else stays quiet and native.

### Rejected alternatives

- **Floating vertical tool palette (Figma style):** takes width from screenshots,
  which are usually wide.
- **Pink as the app accent:** clashes with red annotations (selection handles become
  hard to see) and isn't native. Pink moves to annotations only.

## Work items

| Item | Findings | When |
|---|---|---|
| D1 Theme: follow system light/dark in window and CSS, colour tokens | 1, 5, 10 | Now (independent of M2-2; part of M5-2 pulled forward) |
| D2 Keep image fitted while resizing until manual zoom | 12 | Now |
| D3 Image edge/shadow; cache checkerboard tile | 11 | Now |
| D4 Title-bar tools, remove logo and footer, canvas badge, quieter buttons | 2, 3, 4, 7, 14 | With M2-2 |
| D5 Native menus and per-platform shortcut labels | 6, 8 | With M2-5 |
| D6 Drop files onto the window; drag-out handle | 9 | D6a drop: now; D6b drag-out: with M2-9 |
| D7 Tool-aware cursors and panning | 13 | With M2-2 |
