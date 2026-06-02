import { useEffect, useSyncExternalStore } from "react";
import type { AppSettings } from "../shell/SettingsDialog";

const STORAGE_KEY = "skill-library-settings";

function getSnapshot(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch { /* ignore */ }
  return { theme: "system", accentColor: "blue", language: "auto", proxyMode: "none", proxyUrl: "", requestTimeout: 30, aiProvider: "none", aiConfigs: { openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-5.5" }, anthropic: { baseUrl: "https://api.anthropic.com/v1", model: "claude-opus-4-6" } } };
}

let cachedSettings = getSnapshot();
const listeners = new Set<() => void>();

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => { listeners.delete(cb); };
}

// Listen for storage changes from SettingsDialog
window.addEventListener("storage", () => {
  cachedSettings = getSnapshot();
  listeners.forEach((cb) => cb());
});

// Custom event for same-tab updates
window.addEventListener("skill-library-settings-changed", () => {
  cachedSettings = getSnapshot();
  listeners.forEach((cb) => cb());
});

export function notifySettingsChanged() {
  window.dispatchEvent(new CustomEvent("skill-library-settings-changed"));
}

function getSystemDark() {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/**
 * Applies theme (dark/light) and accent color classes to <html>.
 * Call once at app root.
 */
export function useTheme() {
  const settings = useSyncExternalStore(subscribe, () => cachedSettings);

  useEffect(() => {
    const root = document.documentElement;

    // Theme
    const isDark =
      settings.theme === "dark" || (settings.theme === "system" && getSystemDark());
    root.classList.toggle("dark", isDark);

    // Accent
    root.classList.remove("accent-purple", "accent-green", "accent-orange");
    if (settings.accentColor && settings.accentColor !== "blue") {
      root.classList.add(`accent-${settings.accentColor}`);
    }
  }, [settings.theme, settings.accentColor]);

  // Listen for system theme changes when mode is "system"
  useEffect(() => {
    if (settings.theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => {
      document.documentElement.classList.toggle("dark", mq.matches);
    };
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [settings.theme]);

  return settings;
}
