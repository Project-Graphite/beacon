# Beacon design

Beacon's look is refined Graphite: the Project Graphite dark identity, calmer and roomier, with a
matching light theme. Both the panel and the native overlay follow the Windows app theme; the tray
icon follows the taskbar theme.

The panel is an operating surface. People glance at it, answer, and leave, so it favours familiar
controls, steady layout and short motion over expression. Colour carries state, never decoration.

## Tokens

Defined once in `app/src/index.css` (`@theme`, light values under `prefers-color-scheme: light`) and
mirrored for the native overlay in `app/src-tauri/src/raster.rs` (`Palette`). Light-theme accents are text-grade
(at least 4.5:1 on `surface`); the overlay's marks use slightly brighter mark-grade accents.

| Token | Dark | Light | Use |
|---|---|---|---|
| `paper` | `#111114` | `#f6f6f8` | Panel background |
| `surface` | `#19191d` | `#ececf0` | Hover, selected row, code and command blocks |
| `raised` | `#1d1d22` | `#ffffff` | Cards, menus, inputs, outline buttons |
| `line` | `#2b2b32` | `#dcdce2` | Borders |
| `line-soft` | `#202026` | `#e6e6eb` | Row and section dividers |
| `ink` | `#ececef` | `#18181b` | Text, primary buttons |
| `muted` | `#9b9ba5` | `#55555f` | Secondary text |
| `faint` | `#8b8b95` | `#62626c` | Labels, times, hints |
| `wip` | `#e0ad52` | `#8a5a0a` | Needs you, stalled, focus of attention |
| `live` | `#4cc283` | `#187045` | Done |
| `danger` | `#f2a09a` | `#ad3a31` | Error, destructive |

Text sizes: `title` 14 px, `body` 13 px, `secondary` 12 px, `micro` 11.5 px, all in Segoe UI
Variable. Numbers that change (times, counts, tokens) use tabular figures, not a monospace face;
monospace is only for commands, paths in code and diffs.

Radii: fields and buttons 8 px, cards 12 px, the panel 14 px. Depth comes from a soft offset shadow
(`shadow-card`, `shadow-popover`) and a 1 px top sheen on the panel, never a zero-offset glow.

## Layout

- The panel is 376 px wide and grows with its content up to the work area.
- Gutters are 16 px. A session row is 52 px: a 28 px avatar, the title, and one line of progress.
  A waiting request sits under its row, aligned with the title.
- Group headings are 11.5 px medium in `faint`, with more space above than below.
- Settings rows are at least 40 px, label on the left, control on the right; help is in the
  tooltip, so rows stay one line.

## Components

- Buttons: filled (`ink` on `paper`) for the one primary action of a card, outline (`raised` with a
  hairline) for the rest, text buttons for low-weight actions such as Undo or Cancel. Regular is 28 px
  high, compact 26 px.
- Icon buttons are 28 px with a 16 px icon and no border.
- Controls: segmented control (inset `surface` track, the chosen segment lifted on `raised`),
  switch (36 × 20, spring knob), select (28 px trigger, 32 px options in a popover), slider, and the
  shortcut recorder, which shows the shortcut as keycaps and records the next key combination.
- A needs-you row is tinted with `wip` at 7%; an alert never uses a coloured side border.
- Request cards are `raised` with a hairline and `shadow-card`; a destructive request gets a `danger`
  border and a second-press Allow.

## Motion

Motion says what changed. It is short, eases out, and starts from a visible state.

- Panel: 180 ms entrance from its edge, 140 ms exit.
- Rows enter, leave and reorder with a FLIP move; views slide between list, session and settings.
- Hover and state colours fade over 120 to 200 ms; switches use a light spring.
- Looping marks (working arc, needs-you halo, avatar blink) step at 60 frames a second and pause
  while the panel or a row is off screen. The native pill and tab never draw faster than 60 frames
  a second and draw nothing while nothing changes.
- `prefers-reduced-motion` removes animations and transitions.

## Overlay

- The edge tab is a 4 px strip on a screen edge, coloured by the overall state.
- The pill is a capsule with one mark per session, a needs-you count and a chevron, lifted by a soft
  2 px shadow. With `Pill background: None` only the marks float, each with its own shadow.
- `Show it: Only when Claude needs you` with `Reveal on hover` keeps the overlay invisible until a
  session needs you, and fades it in when the pointer rests on its spot.
- State marks: rotating arc (working), amber circle with a halo (needs you), pause glyph (stalled),
  green check (done), red triangle (error), hollow ring (idle).

## Icon

Icons carry no colour. The app mark is a pulse: a bright core with two full rings fading outward, the
same halo a waiting session shows, in white on a dark graphite square with a soft top highlight. The
tray icon draws the same mark in the taskbar's own ink, with no plate, and adds a small solid badge
at the top right while a session needs you or has an error.
