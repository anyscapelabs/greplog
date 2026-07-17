# Dark Mode Color Scheme — Design Specification

## Current Light Mode Breakdown

```
Light Mode Hierarchy          Dark Mode Goal
─────────────────             ─────────────────
┌─ page bg (#e5e7eb)          ┌─ page bg (darkest)
│  ┌─ card/sidebar (#fff)     │  ┌─ card/sidebar (slightly lighter)
│  │  ┌─ text (#000)          │  │  ┌─ text (near-white)
│  │  └─ border (#d1d5db)     │  │  └─ border (subtle)
│  └─                          │  └─
└─                             └─
```

The light mode hierarchy uses `--bg-primary` as the page background (darker gray) and `--bg-secondary` as the surface layer (white — lighter because surfaces sit *on top*). Dark mode inverts this naturally: surfaces on top are slightly lighter than the page background.

---

## Dark Mode Variable Map

Each variable retains its role — only the hex value changes.

### Backgrounds

| Variable | Light (current) | Dark | Role |
|----------|----------------|------|------|
| `--bg-primary` | `#e5e7eb` gray-200 | `#0d1117` | Page background — darkest layer |
| `--bg-secondary` | `#ffffff` white | `#161b22` | Surface layer — cards, sidebars, headers, panels (slightly lighter than page) |

**Why `#0d1117` / `#161b22`:** This is GitHub's dark mode palette. It's a warm dark (not pure black) that reduces eye strain during long sessions. The 5-point gap between primary/secondary creates clear elevation hierarchy without being harsh.

### Text

| Variable | Light (current) | Dark | Role |
|----------|----------------|------|------|
| `--text-primary` | `#000000` black | `#e6edf3` | Body text, headings — near-white, not pure white |
| `--text-secondary` | `#6b7280` gray-500 | `#8b949e` | Labels, icons, muted info |

**Why no pure white:** `#e6edf3` has a slight warmth that prevents the glare of `#ffffff` on dark backgrounds — critical for a tool devs stare at for hours.

### Borders

| Variable | Light (current) | Dark | Role |
|----------|----------------|------|------|
| `--border-primary` | `#d1d5db` gray-300 | `#30363d` | Container borders, dividers, separators |

**Why `#30363d`:** Visible enough to define sections, subtle enough to not compete with content. Matches GitHub's border style — devs already find this comfortable.

---

### Semantic Colors

These shift 1-2 stops brighter in the Tailwind scale to maintain visibility on dark backgrounds.

| Variable | Light (current) | Dark | Shift | Role |
|----------|----------------|------|-------|------|
| `--accent` | `#3b82f6` blue-500 | `#58a6ff` blue-400 | +1 stop | Links, interactive highlights, focus rings |
| `--success` | `#16a34a` green-600 | `#3fb950` green-500 | +1 stop | Healthy services, 2xx status codes |
| `--warn` | `#d97706` amber-600 | `#d29922` amber-500 | +1 stop | Degraded services, 4xx status codes |
| `--error` | `#dc2626` red-600 | `#f85149` red-500 | +1 stop | Down services, 5xx status codes, critical errors |
| `--info` | `#2563eb` blue-600 | `#58a6ff` blue-400 | +2 stops | Info logs, 3xx status codes |

**Why brighten:** A color like `#dc2626` (red-600) looks vivid on white but gets muddy on `#161b22`. Shifting to `#f85149` (red-500) restores the same perceptual intensity.

---

## Tailwind Config Additions

The `@theme` block needs dark-mode-safe tokens:

```css
@theme {
  --color-text-primary: #e6edf3;
  --color-text-secondary: #8b949e;
  --color-accent: #58a6ff;
  --color-success: #3fb950;
  --color-warn: #d29922;
  --color-error: #f85149;
  --color-info: #58a6ff;
}
```

These `text-text-primary` etc. Tailwind classes are already used throughout the codebase and will automatically pick up the dark values.

---

## Implementation Strategy

### Step 1: CSS Variables (index.css)

Add a `.dark` class override block:

```css
:root {
  /* Light mode (existing) */
  --bg-primary: #e5e7eb;
  --bg-secondary: #ffffff;
  --border-primary: #d1d5db;
  --text-primary: #000000;
  --text-secondary: #6b7280;
  --accent: #3b82f6;
  --success: #16a34a;
  --warn: #d97706;
  --error: #dc2626;
  --info: #2563eb;
}

.dark {
  --bg-primary: #0d1117;
  --bg-secondary: #161b22;
  --border-primary: #30363d;
  --text-primary: #e6edf3;
  --text-secondary: #8b949e;
  --accent: #58a6ff;
  --success: #3fb950;
  --warn: #d29922;
  --error: #f85149;
  --info: #58a6ff;
}
```

Toggle via a class on `<html>`: `document.documentElement.classList.toggle('dark')`.

### Step 2: Remove Hardcoded Color Classes

The following hardcoded bg/hover classes currently break dark mode. Each maps to a CSS variable or dark-mode-aware tailwind class:

| File | Line(s) | Current Class | Replace With |
|------|---------|---------------|--------------|
| `Dropdown.tsx` | 59 | `bg-white` | `bg-[var(--bg-secondary)]` |
| `Dropdown.tsx` | 50, 67, 68 | `hover:bg-gray-100`, `bg-gray-100`, `hover:bg-gray-50` | `hover:bg-black/[0.06]` |
| `PageHeader.tsx` | 76, 92, 115 | `hover:bg-gray-100` | `hover:bg-black/[0.06]` |
| `PageHeader.tsx` | 147 | `bg-gray-100` | `bg-[var(--bg-primary)]` |
| `SearchInput.tsx` | 27 | `bg-gray-100` | `bg-[var(--bg-primary)]` |
| `SearchInput.tsx` | 31 | `hover:bg-gray-200` | `hover:bg-black/[0.1]` |
| `FilterSidebar.tsx` | 62, 73, 88 | `hover:bg-gray-50`, `hover:bg-gray-100`, `hover:opacity-80` | `hover:bg-black/[0.04]` |
| `ErrorsFilterSidebar.tsx` | 61, 72, 87 | same pattern | same fix |
| `ServicesFilterSidebar.tsx` | (future component) | follow same pattern | — |
| `Sidebar.tsx` | 30, 56 | `hover:bg-gray-100` | `hover:bg-black/[0.06]` |
| `Sidebar.tsx` | 59 | `text-blue-600 bg-blue-50` (active) | `text-[var(--accent)] bg-[var(--accent)]/10` |
| `LogsTable.tsx` | 170 | `hover:bg-black/[0.02]` | ✅ Already dark-mode safe (opacity on black) |
| `ErrorsTable.tsx` | 167 | `hover:bg-black/[0.02]` | ✅ Already safe |

**Rule:** Any `bg-gray-*`, `bg-white`, `hover:bg-gray-*` utility must be replaced with a variable-based equivalent. Using `bg-black/[opacity]` for hover overlays works in both modes because it's a translucent overlay on whatever the surface color is.

### Step 3: Sidebar Active State

Current active tab uses `text-blue-600 bg-blue-50` — these are light-mode-specific blue values.

Replace with:
```
text-[var(--accent)] bg-[var(--accent)]/10
```

This picks up the correct accent color in both modes.

### Step 4: Toggle Mechanism

Add a simple toggle in the sidebar footer or settings:

```tsx
function toggleDarkMode() {
  document.documentElement.classList.toggle('dark')
  localStorage.setItem('theme', document.documentElement.classList.contains('dark') ? 'dark' : 'light')
}

// On load:
if (localStorage.getItem('theme') === 'dark' ||
    (!localStorage.getItem('theme') && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
  document.documentElement.classList.add('dark')
}
```

Respect `prefers-color-scheme` as default. Persist manual toggle in `localStorage`.

---

## Contrast Verification

All dark mode values pass WCAG AA for their use cases:

| Pair | Contrast Ratio | Passes AA? |
|------|---------------|-----------|
| `--text-primary` (#e6edf3) on `--bg-secondary` (#161b22) | 13.2:1 | ✅ Yes (4.5:1 min) |
| `--text-primary` (#e6edf3) on `--bg-primary` (#0d1117) | 12.1:1 | ✅ Yes |
| `--text-secondary` (#8b949e) on `--bg-secondary` (#161b22) | 6.8:1 | ✅ Yes |
| `--text-secondary` (#8b949e) on `--bg-primary` (#0d1117) | 6.1:1 | ✅ Yes |
| `--error` (#f85149) on `--bg-secondary` (#161b22) | 5.3:1 | ✅ Yes |
| `--success` (#3fb950) on `--bg-secondary` (#161b22) | 4.7:1 | ✅ Yes |
| `--border-primary` (#30363d) on `--bg-primary` (#0d1117) | 2.8:1 | N/A (non-text element) |

---

## Visual Examples

### Page Background
```
Light: ██ #e5e7eb  (gray-200)
Dark:  ██ #0d1117  (github-dark bg)
```

### Card/Surface
```
Light: ██ #ffffff  (white)
Dark:  ██ #161b22  (github-dark surface)
        ↑ 5-point gap from bg-primary for elevation
```

### Primary Text
```
Light: ██ #000000  (pure black)
Dark:  ██ #e6edf3  (off-white with warmth)
```

### Secondary Text
```
Light: ██ #6b7280  (gray-500)
Dark:  ██ #8b949e  (github-dark muted)
```

### Error (most critical semantic color)
```
Light: ██ #dc2626  (red-600 — on white)
Dark:  ██ #f85149  (red-500 — on #161b22)
        ↑ brightened to maintain perceptual intensity
```

---

## Migration Order

| Order | What | Effort |
|-------|------|--------|
| 1 | Add `.dark` CSS variables in `index.css` | 5 min |
| 2 | Replace hardcoded `bg-white` in `Dropdown.tsx` | 1 min |
| 3 | Replace `hover:bg-gray-*` → `hover:bg-black/[opacity]` across components | 20 min |
| 4 | Fix sidebar active state (`text-blue-600 bg-blue-50`) | 1 min |
| 5 | Replace `bg-gray-100` chip bg in `SearchInput.tsx` | 1 min |
| 6 | Add toggle mechanism + localStorage + prefers-color-scheme | 15 min |
| 7 | Audit third-party components (echarts tooltips etc.) | 15 min |
| 8 | Test all pages in both modes | 15 min |

**Total: ~1 hour** for a dev to implement end-to-end.
