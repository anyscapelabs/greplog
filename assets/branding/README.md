# Greplog Brand Assets

## Directory Structure

```
assets/branding/
├── brand-colors.json              # Official color palette
├── README.md                      # This file
└── logo/
    ├── icon/                      # Icon only (no text)
    │   ├── icon-black.svg
    │   ├── icon-blue.svg
    │   └── icon-white.svg
    ├── wordmark/                  # Wordmark only (no icon)
    │   ├── wordmark-black.svg
    │   ├── wordmark-blue.svg
    │   └── wordmark-white.svg
    ├── app-icon/                  # App icon (square, for app stores)
    │   ├── app-icon-black.svg
    │   └── app-icon-blue.svg
    └── favicon/                   # Favicon variants (small, for browser tabs)
        ├── favicon-black.svg
        ├── favicon-blue.svg
        └── favicon-purple.svg     # Primary brand color (default for dashboard)
```

## Dashboard Sync

The dashboard uses `dashboard/public/favicon.svg` — keep it synced with
`assets/branding/logo/favicon/favicon-purple.svg`.

```sh
cp assets/branding/logo/favicon/favicon-purple.svg dashboard/public/favicon.svg
```

## Colors

Primary brand color: `#863bff` (purple)
See `brand-colors.json` for the full palette.
