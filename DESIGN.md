# Design System — havn

## Product Context

- **What this is:** A local developer dashboard that maps running dev-server processes
  to project directories. CLI + web UI at `localhost:9390`.
- **Who it's for:** Developers running 5+ concurrent dev servers who've lost track of
  which port belongs to which project.
- **Space:** Developer tooling (peers: Linear, Warp, Raycast). Runs entirely offline.
- **Project type:** Utility dashboard — data-dense, keyboard-accessible, two-column max.

---

## Aesthetic Direction

- **Direction:** Industrial / Utilitarian
- **Decoration level:** Minimal — typography and spacing carry the full aesthetic weight.
  No decorative elements, no gradients, no illustrations.
- **Mood:** Precision instrument. The interface should feel like something that was built
  to work, not to impress. The developer opens `localhost:9390` and immediately knows
  where things are — not because the design is flashy, but because it's disciplined.
- **Anti-patterns to avoid:**
  - No glassmorphism or frosted surfaces
  - No shadow theatrics (only subtle depth separation)
  - No gradient buttons
  - No rounded-pill badges for framework labels — rectangular code-stamps only
  - No centered layouts; left-aligned, scannable top-to-bottom

---

## Typography

**Complete type system: IBM Plex** — paired sans and mono from the same family.
Cohesive at every scale, strong developer-tool pedigree, tabular figures on the mono.

| Role | Face | Weight | Size | Notes |
|------|------|--------|------|-------|
| Wordmark | IBM Plex Mono | 600 | 14–16px | `havn` header, always lowercase |
| Project names | IBM Plex Mono | 500 | 13–15px | Primary visual event in every row |
| Port / uptime data | IBM Plex Mono | 400 | 11–13px | Tabular-nums, always mono |
| Start commands | IBM Plex Mono | 400 | 11px | `$ npm run dev` style |
| UI labels / body | IBM Plex Sans | 400 | 13px | Stats, descriptions, toast messages |
| Buttons / badges | IBM Plex Sans | 500 | 11–12px | Controls and section labels |
| Section labels | IBM Plex Mono | 600 | 10px | UPPERCASE, `letter-spacing: 0.1em` |

**Loading (CDN):**
```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@300;400;500;600&family=IBM+Plex+Sans:wght@300;400;500;600;700&display=swap" rel="stylesheet">
```

**Type scale:**
```
24px — wordmark / display
16px — project names (primary data)
13px — body text, UI labels
12px — secondary data (ports, uptime)
11px — buttons, badges, fine print
10px — section labels (UPPERCASE)
```

---

## Color

**Approach:** Restrained — one accent color, the rest is semantic. Color is rare and
meaningful; using it signals importance.

### Dark mode (default)

```css
:root {
  /* Backgrounds */
  --bg-primary:    #0C0D10;  /* near-black with a blue lean */
  --bg-secondary:  #141619;  /* surface — rows, cards */
  --bg-row-hover:  #1C1F24;  /* hover state */

  /* Text */
  --text-primary:  #E2E0D9;  /* warm off-white — intentional temperature */
  --text-secondary:#9A9891;
  --text-muted:    #525560;

  /* Borders */
  --border:        #1F2229;  /* whisper-thin — borders shouldn't shout */
  --border-2:      #2A2E38;  /* slightly stronger for interactive elements */

  /* Accent (phosphor green — one accent, used sparingly) */
  --accent:        #7AFFA3;  /* active ports, live dot, scan badge */
  --accent-dim:    #1A3D2B;  /* accent background tint */

  /* Semantic */
  --danger:        #EF4444;
  --danger-dim:    #3D1515;
  --warning:       #F59E0B;
  --success:       #22C55E;
  --star:          #FACC15;  /* favorites */
}
```

### Light mode

```css
@media (prefers-color-scheme: light) {
  :root {
    --bg-primary:    #F2F0E8;  /* off-paper, not clinical white */
    --bg-secondary:  #FAFAF7;
    --bg-row-hover:  #F0EEE6;
    --text-primary:  #1A1916;
    --text-secondary:#5C5950;
    --text-muted:    #9A9790;
    --border:        #DDDAD0;
    --border-2:      #CCC9BF;
    --accent:        #059669;  /* teal — phosphor green reads better as teal on light */
    --accent-dim:    #D1FAE5;
    --danger:        #DC2626;
    --danger-dim:    #FEE2E2;
    --warning:       #D97706;
    --success:       #16A34A;
    --star:          #CA8A04;
  }
}
```

**Accent usage rules:**
- Active port numbers in the project row
- Live indicator dot in the header
- `scan badge` showing last-scan timestamp
- Input focus ring
- **Nothing else.** If you're reaching for the accent to style something else, the
  answer is probably `--text-secondary` or a semantic color instead.

**Framework badge colors:** Each framework has its own color key. These are the only
place non-semantic color proliferates. Keep them dim (22% opacity background, full
opacity text). See `dashboard/app.js: FRAMEWORK_COLORS` for the canonical map.

---

## Spacing

- **Base unit:** 8px
- **Density:** Compact-comfortable — this is a data tool, not a marketing page.
  Padding is generous enough for tappability (min 44px touch targets) but tight
  enough that 10 rows are scannable without scrolling.

```
2xs:  4px   — icon gap, badge inner padding
xs:   8px   — intra-row gap, small padding
sm:   12px  — element padding (buttons, inputs)
md:   16px  — row horizontal padding
lg:   24px  — section gap
xl:   32px  — major section separation
2xl:  48px  — empty state
```

**Border radius:**
```
none (0)     — table-like row dividers
sm (3px)     — badges, small chips
md (4px)     — buttons, inputs, toasts
lg (8px)     — cards, dashboard container
```

---

## Layout

- **Approach:** Grid-disciplined — strict left-alignment, consistent column widths,
  predictable scan path top-to-bottom.
- **Max content width:** None (fills viewport — this is a local utility, not a
  marketing page; the developer may have it in a quarter-screen panel).
- **Row anatomy (left → right):**
  `[fav ★] [project-name 148px min] [fw-badge 52px] [ports 80px] [uptime 48px] [→ actions]`
- **Mobile:** Wrap badge + uptime to second line; hide uptime on very small screens.

---

## Motion

- **Approach:** Minimal-functional — animations exist only to aid comprehension, never
  for decoration.
- **Rules:**
  - Row fade-in on initial load: `fadeIn 200ms ease-out` (staggered by index × 50ms)
  - Row hover: background color transition `150ms`
  - Buttons/inputs: border-color / background `120–150ms`
  - Toast slide-up: `slideUp 200ms ease-out`
  - **No** looping animations except the live dot pulse (opacity 1↔0.4, 2s, signals
    liveness)

```css
/* Easing */
--ease-enter: ease-out;
--ease-exit:  ease-in;
--ease-move:  ease-in-out;

/* Durations */
--dur-micro:  100ms;   /* hover color transitions */
--dur-short:  150ms;   /* button state changes */
--dur-medium: 200ms;   /* row entry, toast */
--dur-long:   300ms;   /* reconnect banner */
```

---

## V3 North Star

These are the design directions proposed by outside design voices (Codex + subagent).
They represent the target v3 redesign — not v2 scope, but the direction to aim toward.

### Codex — Signal Board (Industrial Control Room)

> "Industrial editorial control room: brushed-steel calm, signal-orange urgency, and
> the nervous kinetic energy of too many local processes barely held in formation."

- **Layout:** Left rail (28–32%) with oversized live counts (`07 RUNNING · 13 PORTS`).
  Right field: dense staggered horizontal bands. The page scrolls like a control ledger.
- **Accent:** Signal orange `#FF6A1A` instead of phosphor green
- **Typography:** `IBM Plex Sans Condensed` for labels, `JetBrains Mono` for data
- **Badges:** Rectangular code-stamps, mostly monochrome, one keyed accent edge

### Subagent — Mission Control (Hold Gesture)

> "Quiet recognition followed immediately by mild envy. The developer opens
> localhost:9390 and thinks: someone cared about this the way I care about my projects."

- **Layout:** Stacked cards — abandon the row/table metaphor. Top line: name + ports.
  Second line: framework + uptime. Actions appear on hover only.
- **Kill UX:** 600ms hold gesture with circular progress ring (green → red).
  No modal, no toast. The feedback lives in the gesture itself.
- **Background:** Off-paper `#F2F0E8` (light), blue-black `#0C0D10` (dark)

---

## Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-27 | IBM Plex as complete type system | Paired sans + mono, developer-tool pedigree, not overused, available on Google Fonts |
| 2026-03-27 | Phosphor green `#7AFFA3` accent (dark) / teal `#059669` (light) | Single accent, terminal-cursor reference, strong contrast without aggression |
| 2026-03-27 | Off-paper `#F2F0E8` light bg / blue-black `#0C0D10` dark bg | Warm light bg reads as intentional; cool-dark bg pairs with phosphor accent |
| 2026-03-27 | Minimal-functional motion only | Data tool — animation must aid comprehension, never decorate |
| 2026-03-27 | Row layout stays for v2; stacked cards / left-rail is v3 | Row layout works; v3 redesign is a larger UX commitment, see North Star above |
| 2026-03-27 | No primary interactive accent beyond semantic colors | Everything is either semantic (danger/success/warning) or neutral; accent = liveness signal only |
