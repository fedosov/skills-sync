import type { RefreshIntervalMinutes, UiSettings } from "../types";

const STORAGE_KEY = "agent-sync.ui.settings.v1";

export const DEFAULT_UI_SETTINGS: UiSettings = {
  refreshIntervalMinutes: 0,
  lastActiveTab: "skills",
};

function isRefreshIntervalMinutes(
  value: unknown,
): value is RefreshIntervalMinutes {
  return value === 0 || value === 5 || value === 15 || value === 30;
}

function parseSettings(raw: unknown): UiSettings {
  if (!raw || typeof raw !== "object") {
    return { ...DEFAULT_UI_SETTINGS };
  }
  const candidate = raw as Partial<UiSettings>;

  return {
    refreshIntervalMinutes: isRefreshIntervalMinutes(
      candidate.refreshIntervalMinutes,
    )
      ? candidate.refreshIntervalMinutes
      : DEFAULT_UI_SETTINGS.refreshIntervalMinutes,
    lastActiveTab:
      candidate.lastActiveTab === "skills" ||
      candidate.lastActiveTab === "subagents" ||
      candidate.lastActiveTab === "mcp" ||
      candidate.lastActiveTab === "agents"
        ? candidate.lastActiveTab
        : DEFAULT_UI_SETTINGS.lastActiveTab,
  };
}

export function loadUiSettings(): UiSettings {
  if (typeof window === "undefined") {
    return { ...DEFAULT_UI_SETTINGS };
  }
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return { ...DEFAULT_UI_SETTINGS };
    }
    return parseSettings(JSON.parse(raw));
  } catch {
    return { ...DEFAULT_UI_SETTINGS };
  }
}

export function saveUiSettings(settings: UiSettings): void {
  if (typeof window === "undefined") {
    return;
  }
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Ignore storage failures in restricted environments.
  }
}
