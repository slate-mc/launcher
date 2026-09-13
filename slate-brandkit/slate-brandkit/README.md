# slate brand kit

Start with `guidelines/slate-brand-guide.pdf` for the visual guide and `guidelines/slate-brand-system.md` for the complete written rules.

- `logos/svg/`: 20 outlined vector masters (four compositions, five colors).
- `logos/png/`: 20 transparent, 2048px-wide exports generated from the vector masters.
- `icons/`: square app icon SVG, PNGs from 16 to 1024px, Windows ICO and macOS ICNS.
- `social/`: announcement card, banner and avatar, each supplied as PNG and scalable PDF.
- `tokens/`: Tailwind v4 CSS, JSON tokens, calculated contrast pairs.
- `fonts/`: Manrope variable and IBM Plex Mono fonts with original licenses.
- `reference/`: generated concept board and generation notes.

Use the production SVGs and token files as the source of truth. The generated concept board is a visual reference; its incidental colors, lettering and mockups are not exact masters. SVG logo text is already outlined and needs no installed font. Font URLs in the supplied CSS are relative to this package and may need adjustment in your app.

Always write the brand name as **slate** in lowercase.
