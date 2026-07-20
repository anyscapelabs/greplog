# Branding Assets Strategy

## Current State

| Location | File | Purpose |
|----------|------|---------|
| `dashboard/public/favicon.svg` | Purple "globe" icon | Browser tab favicon ✅ |
| `dashboard/public/icons.svg` | Social media icons (Bluesky, Discord, GitHub, X, etc.) | Dashboard footer/links ✅ |
| `dashboard/index.html` | Title still `"dashboard"` | ❌ Should be `"Greplog"` |
| Repo root | ❌ No `branding/` or `assets/` directory | Missing |

---

## Recommendation: Two Locations

### 1. Repo Root: `/assets/branding/` — Source of Truth

All brand assets go here. This is the canonical location for the community, blog posts, talks, and any external use.

```
assets/branding/
├── logo/
│   ├── greplog-logo.svg              # Full logo (icon + wordmark)
│   ├── greplog-icon.svg              # Icon only (the purple globe)
│   ├── greplog-icon-white.svg        # White variant for dark backgrounds
│   └── greplog-wordmark.svg          # Wordmark only
├── og-image.png                      # Social share card (1200×630)
├── brand-colors.json                 # Official color palette
└── README.md                         # Brief usage guide for contributors
```

`.gitignore` doesn't exclude `assets/` — these should be committed.

### 2. `dashboard/public/` — Runtime Assets

Only what the dashboard app needs at runtime. These are referenced by the Vite build.

```
dashboard/public/
├── favicon.svg              # ✅ Already exists (copy of greplog-icon.svg)
├── icons.svg                # ✅ Already exists
├── logo.svg                 # NEW — greplog-logo.svg for dashboard header
└── og-image.png             # NEW — Open Graph social share image
```

Keep `dashboard/public/` as a **working copy** — whenever brand assets are updated in `assets/branding/`, copy the relevant files here.

---

## What Needs Fixing

### index.html

```diff
- <title>dashboard</title>
+ <title>Greplog</title>
```

Add OG meta tags for social sharing:

```html
<meta property="og:title" content="Greplog" />
<meta property="og:description" content="Open-source observability for AI-assisted coders and small dev teams" />
<meta property="og:image" content="/og-image.png" />
```

### Brand Colors (for `brand-colors.json`)

Derived from the existing favicon and dashboard:

```json
{
  "primary": "#863bff",
  "primary_dark": "#7e14ff",
  "primary_light": "#ede6ff",
  "accent_blue": "#3b82f6",
  "success_green": "#16a34a",
  "warn_amber": "#d97706",
  "error_red": "#dc2626",
  "info_blue": "#2563eb",
  "bg_light": "#ffffff",
  "bg_dark": "#0d1117",
  "text_primary_light": "#000000",
  "text_primary_dark": "#e6edf3"
}
```

---

## Which Assets Are Needed Immediately

| Priority | Asset | Location | Why |
|----------|-------|----------|-----|
| 🔴 High | Fix `<title>` | `dashboard/index.html` | Currently shows "dashboard" on the tab |
| 🔴 High | `favicon.svg` | Already in `dashboard/public/` | ✅ Done |
| 🟡 Medium | `logo.svg` for header | `dashboard/public/` + `assets/branding/logo/` | Dashboard branded header |
| 🟡 Medium | `og-image.png` | Both locations | Social sharing when link is posted |
| 🟢 Low | `brand-colors.json` | `assets/branding/` | Contributor reference |
| 🟢 Low | `assets/branding/` directory | Repo root | Community discoverability |

---

## Summary

| Location | Purpose | Synced? |
|----------|---------|---------|
| `assets/branding/` | Source of truth — logo variants, brand colors, OG image source | Always the canonical version |
| `dashboard/public/` | Runtime — favicon, logo SVG, OG image for build output | Copy from `assets/branding/` on update |

**Rule:** Never put a brand asset in only one location. If it's needed in the dashboard, it belongs in `assets/branding/` first, then copied to `dashboard/public/`.
