import { renderHook, act } from "@testing-library/react";
import { describe, expect, it, beforeEach } from "vitest";
import { useFavorites } from "./useFavorites";

const STORAGE_KEY = "agent-sync.favorites.v1";

beforeEach(() => {
  window.localStorage.removeItem(STORAGE_KEY);
});

describe("useFavorites", () => {
  it("starts with empty sets when no stored data", () => {
    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.size).toBe(0);
    expect(result.current.favorites.mcp.size).toBe(0);
    expect(result.current.favorites.agents.size).toBe(0);
  });

  it("toggles a favorite on and off", () => {
    const { result } = renderHook(() => useFavorites());

    act(() => result.current.toggleFavorite("subagents", "sub-1"));
    expect(result.current.favorites.subagents.has("sub-1")).toBe(true);

    act(() => result.current.toggleFavorite("subagents", "sub-1"));
    expect(result.current.favorites.subagents.has("sub-1")).toBe(false);
  });

  it("persists to localStorage", () => {
    const { result } = renderHook(() => useFavorites());

    act(() => result.current.toggleFavorite("mcp", "mcp-key-1"));

    const raw = window.localStorage.getItem(STORAGE_KEY) ?? "{}";
    expect(raw).toContain("mcp-key-1");
  });

  it("reads persisted data on mount", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ subagents: ["a"], mcp: [], agents: ["b"] }),
    );

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.has("a")).toBe(true);
    expect(result.current.favorites.agents.has("b")).toBe(true);
  });

  it("handles corrupt localStorage gracefully", () => {
    window.localStorage.setItem(STORAGE_KEY, "not-valid-json");

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.size).toBe(0);
  });
});
