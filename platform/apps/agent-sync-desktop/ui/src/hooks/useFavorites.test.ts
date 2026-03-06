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
      JSON.stringify({
        subagents: ["sub-1"],
        mcp: ["mcp-1"],
        agents: ["agent-1"],
      }),
    );

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.has("sub-1")).toBe(true);
    expect(result.current.favorites.mcp.has("mcp-1")).toBe(true);
    expect(result.current.favorites.agents.has("agent-1")).toBe(true);
  });

  it("falls back to empty sets for malformed JSON", () => {
    window.localStorage.setItem(STORAGE_KEY, "not-valid-json");

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.size).toBe(0);
    expect(result.current.favorites.mcp.size).toBe(0);
    expect(result.current.favorites.agents.size).toBe(0);
  });

  it("falls back to empty arrays when a stored field is not an array", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        subagents: "bad",
        mcp: 123,
        agents: null,
      }),
    );

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.size).toBe(0);
    expect(result.current.favorites.mcp.size).toBe(0);
    expect(result.current.favorites.agents.size).toBe(0);
  });

  it("falls back to empty arrays when an array contains non-string members", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        subagents: ["ok", 42, null, true],
        mcp: [{}],
        agents: ["agent-1", ["nested"]],
      }),
    );

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.size).toBe(0);
    expect(result.current.favorites.mcp.size).toBe(0);
    expect(result.current.favorites.agents.size).toBe(0);
  });

  it("ignores unknown keys in persisted data", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        subagents: ["sub-1"],
        mcp: [],
        agents: ["agent-1"],
        unknown: ["ignored"],
      }),
    );

    const { result } = renderHook(() => useFavorites());
    expect(result.current.favorites.subagents.has("sub-1")).toBe(true);
    expect(result.current.favorites.agents.has("agent-1")).toBe(true);
    expect(result.current.favorites.mcp.size).toBe(0);
  });
});
