import { useCallback, useState } from "react";

const STORAGE_KEY = "agent-sync.favorites.v1";

type FavoritesKind = "subagents" | "mcp" | "agents";

type FavoritesData = Record<FavoritesKind, string[]>;

type UseFavoritesResult = {
  favorites: Record<FavoritesKind, Set<string>>;
  toggleFavorite: (kind: FavoritesKind, id: string) => void;
};

function loadFromStorage(): FavoritesData {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return { subagents: [], mcp: [], agents: [] };
    // eslint-disable-next-line @typescript-eslint/no-unsafe-assignment -- JSON.parse returns unknown
    const parsed: Partial<FavoritesData> = JSON.parse(raw);
    return {
      subagents: Array.isArray(parsed.subagents) ? parsed.subagents : [],
      mcp: Array.isArray(parsed.mcp) ? parsed.mcp : [],
      agents: Array.isArray(parsed.agents) ? parsed.agents : [],
    };
  } catch {
    return { subagents: [], mcp: [], agents: [] };
  }
}

function saveToStorage(data: FavoritesData): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
  } catch {
    // Ignore storage errors in restricted environments.
  }
}

function toSets(data: FavoritesData): Record<FavoritesKind, Set<string>> {
  return {
    subagents: new Set(data.subagents),
    mcp: new Set(data.mcp),
    agents: new Set(data.agents),
  };
}

export function useFavorites(): UseFavoritesResult {
  const [data, setData] = useState<FavoritesData>(loadFromStorage);

  const toggleFavorite = useCallback((kind: FavoritesKind, id: string) => {
    setData((prev) => {
      const list = prev[kind];
      const next = list.includes(id)
        ? list.filter((item) => item !== id)
        : [...list, id];
      const updated = { ...prev, [kind]: next };
      saveToStorage(updated);
      return updated;
    });
  }, []);

  return {
    favorites: toSets(data),
    toggleFavorite,
  };
}
