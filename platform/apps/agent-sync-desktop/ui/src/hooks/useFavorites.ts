import { useCallback, useState } from "react";

const STORAGE_KEY = "agent-sync.favorites.v1";
const EMPTY_FAVORITES_DATA: FavoritesData = {
  subagents: [],
  mcp: [],
  agents: [],
};

type FavoritesKind = "subagents" | "mcp" | "agents";

type FavoritesData = Record<FavoritesKind, string[]>;

type UseFavoritesResult = {
  favorites: Record<FavoritesKind, Set<string>>;
  toggleFavorite: (kind: FavoritesKind, id: string) => void;
};

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every((x) => typeof x === "string");
}

function emptyFavoritesData(): FavoritesData {
  return {
    subagents: [...EMPTY_FAVORITES_DATA.subagents],
    mcp: [...EMPTY_FAVORITES_DATA.mcp],
    agents: [...EMPTY_FAVORITES_DATA.agents],
  };
}

function parseFavoriteIds(value: unknown): string[] {
  return isStringArray(value) ? [...value] : [];
}

function parseFavoritesData(value: unknown): FavoritesData {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return emptyFavoritesData();
  }

  const parsed = value as Partial<Record<FavoritesKind, unknown>>;

  return {
    subagents: parseFavoriteIds(parsed.subagents),
    mcp: parseFavoriteIds(parsed.mcp),
    agents: parseFavoriteIds(parsed.agents),
  };
}

function loadFromStorage(): FavoritesData {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return emptyFavoritesData();
    return parseFavoritesData(JSON.parse(raw));
  } catch {
    return emptyFavoritesData();
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
