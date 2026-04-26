export type ThemePreference = "light" | "dark" | "system";

export function applyThemePreference(theme: ThemePreference) {
  document.documentElement.dataset.theme = theme;
}
