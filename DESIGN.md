# Beacon design

Beacon's look is refined Graphite: the Project Graphite dark identity, calmer and roomier, with a
matching light theme. Both the panel and the native overlay follow the Windows app theme; the tray
icon follows the taskbar theme.

The panel is an operating surface. People glance at it, answer, and leave, so it favours familiar
controls, steady layout and short motion over expression. Colour carries state, never decoration.

## Colors

Defined once in `app/src/index.css` (`@theme`, light values under `prefers-color-scheme: light`).
Light-theme accents are text-grade (at least 4.5:1 on `surface`).

| Token | Dark | Light | Use |
|---|---|---|---|
| `paper` | `#111114` | `#f6f6f8` | Panel background |
| `surface` | `#19191d` | `#ececf0` | Hover, selected row, code and command blocks |
| `raised` | `#1d1d22` | `#ffffff` | Cards, menus, inputs, outline buttons |
| `segment` | `#68686f` | `#ffffff` | The chosen segment of a segmented control |
| `line` | `#2b2b32` | `#dcdce2` | Borders |
| `line-soft` | `#202026` | `#e6e6eb` | Row and section dividers |
| `ink` | `#ececef` | `#18181b` | Text, primary buttons |
| `muted` | `#9b9ba5` | `#55555f` | Secondary text |
| `faint` | `#8b8b95` | `#62626c` | Labels, times, hints |
| `dormant` | `#6e6e78` | `#85858e` | Starting and ended marks |
| `wip` | `#e0ad52` | `#8a5a0a` | Needs you, stalled, focus of attention |
| `wip-tint` | `#e0ad52` | `#a8741c` | Needs-you row tint, used at 7% (11% hovered or selected) |
| `live` | `#4cc283` | `#187045` | Done |
| `danger` | `#f2a09a` | `#ad3a31` | Error, destructive |
| `highlight` | `#d7b24a` | `#7a5c0c` | Variables in commands |
| `sheen` | white at 5% | white at 70% | The panel's 1 px top highlight |

The native overlay mirrors these in `app/src-tauri/src/raster.rs` (`Palette`). It uses the dark
values above unchanged and has no `paper`. In light it draws its capsule in `raised` white, keeps
`line`, `ink` and `muted` as above, and uses mark-grade accents, slightly brighter than the panel's
text-grade ones: `live` `#1f8a54`, `wip` `#a8741c`, `danger` `#c2453b`, with `dormant` `#85858e`.

Avatars are the one exception to colour carrying state: their hues tell sessions apart and mean
nothing else. The six hues (`AVATAR_HUES` in `app/src/lib/avatar.ts`) run from teal through sky,
indigo, violet and magenta to pink at a similar lightness, and stay clear of the amber, green and
red that carry state.

## Typography

One family, Segoe UI Variable Text, with Cascadia Mono for code.

| Token | Size | Line height | Use |
|---|---|---|---|
| `title` | 14 px | 1.35 | Session title in its detail view |
| `body` | 13 px | 1.5 | Row titles, labels, prompt text |
| `secondary` | 12 px | 1.5 | Second lines, buttons, menus, cards |
| `micro` | 11.5 px | 1.45 | Group headings, times, meta lines |

Titles are medium (500) in rows and semibold (600) in the detail view. Numbers that change (times,
counts, tokens) use tabular figures, not a monospace face; monospace is only for commands, paths in
code and diffs.

## Shape and depth

Radii come from five tokens and nothing else:

| Token | Radius | Use |
|---|---|---|
| `mark` | 0.3 rem (about 5 px) | Inline code, keycaps, question headers, the chosen segment inside its 8 px track |
| `field` | 8 px | Fields, buttons, icon buttons, text buttons, menu items, code and command blocks |
| `popover` | 10 px | Menus, the select's option list and the hide notice |
| `card` | 12 px | Request cards |
| `panel` | 14 px | The panel |

Avatars round at a quarter of their size. Depth comes from a soft offset shadow (`shadow-card`,
`shadow-popover`) and a 1 px top sheen on the panel, never a zero-offset glow.

## Layout

- The panel is 376 px wide and grows with its content up to the work area.
- Gutters are 16 px. A session row is 52 px: a 28 px avatar, the title, and one line of progress.
  A waiting request sits under its row, aligned with the title, with its buttons on the row's
  icon column. A task progress bar runs under the text column, below any waiting request.
- Group headings are 11.5 px medium in `faint`, with more space above than below.
- Settings rows are at least 40 px, label on the left, control on the right; help is in the
  tooltip, so rows stay one line.

## Components

- Buttons: filled (`ink` on `paper`) for the one primary action of a card, outline (`raised` with a
  hairline) for the rest, text buttons (at least 24 px) for low-weight actions such as Undo, Cancel
  or More. Regular is 28 px high, compact 26 px. A disabled filled button draws as a disabled
  outline button, so it never reads as an enabled secondary one.
- Icon buttons are 28 px with a 16 px icon and no border. One that cannot act yet, such as hiding
  a row with a pending request, is invisible but keeps its space.
- Every control, text fields and the prompt box included, shows the same focus ring: 2 px of `ink`
  at 70%, offset 2 px.
- Controls: segmented control (inset `surface` track, the chosen segment lifted on `segment`, at
  least 3:1 against the track in dark), switch (36 × 20, spring knob), select (28 px trigger, 32 px
  options in a popover), slider, question radios and checkboxes (native inputs drawn in `faint`,
  filled with `ink` when chosen), and the shortcut recorder, which shows the shortcut as keycaps and
  records the next key combination.
- A needs-you row is tinted with `wip-tint` at 7%, 11% when hovered or selected; an alert never uses
  a coloured side border. The selected row has one cue, a 3 px `ink` bar at its left edge. The
  request the keys act on is set in medium weight in its row and gets a 1 px inset `ink` ring on its
  card; it always belongs to the selected session, so only one row ever looks current.
- Request cards are `raised` with a hairline and `shadow-card`; a destructive request gets a `danger`
  border and a second-press Allow, filled with `danger` while it waits for the second press. A tool
  request's actions sit in two rows: Allow, Deny and Deny with message as buttons, then Always
  allow…, More and Jump as quieter text buttons.
- Commands wrap between words, never after a hyphen, so a flag is not split into what reads as two
  words. A single word or path breaks only when it is wider than the box, at the box edge.

## Motion

Motion says what changed. It is short, eases out, and starts from a visible state.

- Panel: 180 ms entrance from its edge, 140 ms exit.
- Rows enter, leave and reorder with a FLIP move; views slide between list, session and settings.
- Hover and state colours fade over 120 to 200 ms; switches use a light spring.
- Two motions are deliberately longer than short because they move more: a disclosure opens in
  240 ms and closes in 280 ms, and a task progress bar eases to its new value over 500 ms.
- Looping marks (working arc, needs-you halo, avatar blink) step at 60 frames a second and pause
  while the panel or a row is off screen. The native pill and tab never draw faster than 60 frames
  a second and draw nothing while nothing changes.
- `prefers-reduced-motion` removes animations and transitions. Marks keep their shape: the working
  arc stays a still arc, never a full ring that would read as idle.

## Overlay

- The edge tab is a 4 px strip on a screen edge, coloured by the overall state.
- The pill is a capsule with one mark per session, a needs-you count and a chevron, lifted by a soft
  2 px shadow. With `Pill background: None` only the marks float, each with its own shadow.
- `Show it: Only when Claude needs you` with `Reveal on hover` keeps the overlay invisible until a
  session needs you, and fades it in when the pointer rests on its spot.
- State marks: small `dormant` dot (starting), rotating arc (working), amber circle with a halo
  (needs you), pause glyph (stalled), green check (done), red triangle (error), hollow ring (idle).

## Icon

Icons carry no colour. The app mark is a pulse: a bright core with two full rings fading outward, the
same halo a waiting session shows, in white on a dark graphite square with a soft top highlight. The
tray icon draws the same mark in the taskbar's own ink, with no plate, and adds a small solid badge
at the top right while a session needs you or has an error.
