# Bits UI Styling in pop

pop apps use **Bits UI** (`bits-ui`) as the standard primitive layer for interactive controls:
- Dialogs and Modals (`Dialog.Root`)
- Dropdowns and Selects (`Select.Root`)
- Sliders and Faders (`Slider.Root`)
- Checkboxes (`Checkbox.Root`)
- Segmented Views and Tabs (`Tabs.Root`)
- Date Pickers and Calendars (`Calendar.Root` + `Popover.Root`)
- Progress and Meters (`Progress.Root`)
- Tooltips (`Tooltip.Root`)
- Switches and Toggles (`Switch.Root`)

## Why Bits UI?

1. **Zero Default Styles**: Bits UI is completely headless. Unlike styled UI libraries that force a generic SaaS dashboard appearance, Bits UI lets each pop look like a real physical paper document, brass audio hardware, vintage receipt, or retro console.
2. **Accessibility Built-In**: Provides focus restoration, keyboard arrow navigation, screen reader ARIA roles, portal mounting, and scroll management out of the box.
3. **No Homemade Hacks**: Eliminates hand-rolled modal backdrops, broken focus traps, un-styled WebKit `<select>` dropdowns, and range input styling hacks.

---

## Styling Architecture: Data Attributes & CSS Tokens

Style owned elements with app-prefixed classes in plain CSS. Keep data-attribute
states and descendant selectors beside their base rules. Portal content needs
its own class because it mounts outside the trigger's ancestor tree.
See [css.md](css.md) for the authoring pattern.

Bits UI components expose semantic HTML attributes (such as `data-state="open|closed"`, `data-orientation="vertical|horizontal"`, `data-disabled`, `data-selected`).

### 1. Data-Attribute Selectors
Always style Bits UI elements using their standard data attributes:

```css
/* Checkbox Root */
[data-checkbox-root] {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 4px;
  border: 1.5px solid var(--pop-rule);
  background: var(--pop-surface);
  cursor: pointer;
}

[data-checkbox-root][data-state="checked"] {
  background: var(--pop-accent);
  border-color: var(--pop-accent);
  color: #ffffff;
}

/* Vertical Slider (e.g. Audio Faders) */
[data-slider-root][data-orientation="vertical"] {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 28px;
  height: 100%;
}

[data-slider-thumb] {
  cursor: grab;
  outline: none;
}
```

### 2. Runtime Re-Theming with CSS Variables
Define semantic tokens in `:root` so that updating a theme or switching color schemes (e.g., `html[data-theme="dark"]` or `html[data-theme="sepia"]`) instantly updates all Bits UI components:

```css
:root {
  --pop-surface: #fdfbf7;
  --pop-panel: #f3efe6;
  --pop-ink: #1e293b;
  --pop-muted: #64748b;
  --pop-accent: #2563eb;
  --pop-rule: rgba(0, 0, 0, 0.12);
}

[data-theme="dark"] {
  --pop-surface: #0f172a;
  --pop-panel: #1e293b;
  --pop-ink: #f8fafc;
  --pop-muted: #94a3b8;
  --pop-accent: #38bdf8;
  --pop-rule: rgba(255, 255, 255, 0.12);
}
```

---

## Modals & Portals
When using `Dialog.Portal`:
- Mounts outside normal DOM flow into `<body>`.
- Style the `[data-dialog-overlay]` with `position: fixed; inset: 0; backdrop-filter: blur(...);`.
- Style `[data-dialog-content]` with `position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%);`.
- Keep portals in the editor. Synchronous capture views exclude dialogs, menus and their browser dependencies.
