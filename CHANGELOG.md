# Changelog

What changed for users, newest first. Every user-visible change goes under
`Unreleased` in the same commit; a release moves it under its version.

## Unreleased

## 0.6.0 — 2026-11-07

### Important

- **Data moved:** `~/.config/theme-profile/profiles/` is now
  `~/.config/kde-theme-profile/themes/`. Themes, backups and settings move by
  themselves on the first start; nothing is overwritten.
- Applying a theme no longer changes panel and widget layouts, the login screen
  (SDDM) or Neovim.

### Added

- The first install saves your current look as the theme `my-original-look`
  (`--no-original-theme` skips it).
- Going back to a saved theme leaves nothing of the other one: an apply journal
  undoes what the previous theme left, keeping what you changed since.
- What a theme does not specify goes back to the program's default.
- The window's Apply shows what is missing before anything changes: *Download and
  apply* or *Apply without them*. Downloads can be stopped.
- Step log `apply.log`: after a freeze its last line names the step.
- Theme constructor (`theme-profile new`, own Look & Feel), palette with preview,
  dependencies with their sources, automation choices per user, MCP tools, Russian
  translation of data texts, six bundled themes.

### Fixed

- The app crashed at start on a fresh Plasma without colours in `kdeglobals`.
- Applying could leave the desktop without panels: the app killed an apply after 60
  seconds, and `kvantummanager` hung on a missing theme. External tools now have a
  time limit and Plasma always comes back.
- A failed or stalled download no longer blocks the app; one failed item costs only
  itself.
- Icons, cursor, GTK theme, window decoration and Plasma style a machine lacks are
  not switched to.
- Icons in running apps follow a theme change; the GTK theme, icons and cursor reach
  GSettings, which Wayland GTK apps read (a light browser had white text).
- App buttons follow theme changes from anywhere, without a restart.
- A theme without a wallpaper leaves any kind of wallpaper alone.
- Window rules keep KWin 6's `Order=` format.
- Blur no longer spreads over the transparent margins of panels and notifications.

### Removed

- Steam, SDDM, Neovim and the Simple Application Launcher widget are no longer
  themed, listed or installed.
