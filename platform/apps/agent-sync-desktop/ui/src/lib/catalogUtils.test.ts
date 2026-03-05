import { describe, expect, it } from "vitest";
import { sortAndFilter } from "./catalogUtils";

interface Item {
  name: string;
  rank: number;
  tag: string;
}

const items: Item[] = [
  { name: "Charlie", rank: 2, tag: "b" },
  { name: "Alpha", rank: 1, tag: "a" },
  { name: "Bravo", rank: 1, tag: "b" },
];

const compareFn = (a: Item, b: Item) =>
  a.rank - b.rank || a.name.localeCompare(b.name);

const searchFields = (item: Item) => [item.name, item.tag];

describe("sortAndFilter", () => {
  it("sorts items by compareFn when no query", () => {
    const result = sortAndFilter(items, "", compareFn, searchFields);
    expect(result.map((i) => i.name)).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("does not mutate original array", () => {
    const copy = [...items];
    sortAndFilter(items, "", compareFn, searchFields);
    expect(items).toEqual(copy);
  });

  it("filters by query (case-insensitive)", () => {
    const result = sortAndFilter(items, "alpha", compareFn, searchFields);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("Alpha");
  });

  it("trims and lowercases query", () => {
    const result = sortAndFilter(items, "  BRAVO  ", compareFn, searchFields);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("Bravo");
  });

  it("filters by secondary field (tag)", () => {
    const result = sortAndFilter(items, "a", compareFn, searchFields);
    expect(result.map((i) => i.name)).toEqual(["Alpha", "Bravo", "Charlie"]);
  });

  it("returns empty when nothing matches", () => {
    const result = sortAndFilter(items, "zzz", compareFn, searchFields);
    expect(result).toHaveLength(0);
  });

  it("returns empty for empty input", () => {
    const result = sortAndFilter([], "test", compareFn, searchFields);
    expect(result).toHaveLength(0);
  });
});
