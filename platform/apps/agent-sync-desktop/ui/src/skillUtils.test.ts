import { describe, expect, it } from "vitest";
import {
  formatUnixTime,
  normalizeSkillKey,
  pickSelectedSkillKey,
  sortAndFilterSkills,
  type SkillRecord,
} from "./skillUtils";

const skills: SkillRecord[] = [
  {
    id: "1",
    name: "Zeta",
    scope: "project",
    workspace: "/tmp/repo",
    canonical_source_path: "/tmp/repo/.claude/skills/zeta",
    target_paths: [],
    status: "active",
    package_type: "dir",
    skill_key: "zeta",
  },
  {
    id: "2",
    name: "Alpha",
    scope: "global",
    workspace: null,
    canonical_source_path: "/tmp/.claude/skills/alpha",
    target_paths: [],
    status: "active",
    package_type: "dir",
    skill_key: "alpha",
  },
  {
    id: "3",
    name: "Old",
    scope: "global",
    workspace: null,
    canonical_source_path: "/tmp/.claude/skills/old",
    target_paths: [],
    status: "archived",
    package_type: "dir",
    skill_key: "old",
  },
];

describe("normalizeSkillKey", () => {
  it("normalizes mixed symbols", () => {
    expect(normalizeSkillKey("  New Skill ++ Name  ")).toBe("new-skill-name");
  });

  it("returns empty for invalid title", () => {
    expect(normalizeSkillKey("___")).toBe("");
    expect(normalizeSkillKey("   ")).toBe("");
  });
});

describe("formatUnixTime", () => {
  it("returns dash for null", () => {
    expect(formatUnixTime(null)).toBe("-");
  });

  it("returns dash for invalid value", () => {
    expect(formatUnixTime(Number.NaN)).toBe("-");
  });

  it("formats valid unix time", () => {
    expect(formatUnixTime(1700000000)).not.toBe("-");
  });

  it("returns dash for non-finite date values", () => {
    expect(formatUnixTime(Number.POSITIVE_INFINITY)).toBe("-");
  });
});

describe("pickSelectedSkillKey", () => {
  it("keeps preferred when present", () => {
    expect(pickSelectedSkillKey(skills, "zeta", "alpha")).toBe("zeta");
  });

  it("keeps previous when present and preferred missing", () => {
    expect(pickSelectedSkillKey(skills, "missing", "alpha")).toBe("alpha");
  });

  it("falls back to first when none present", () => {
    expect(pickSelectedSkillKey(skills, "missing", "also-missing")).toBe(
      "zeta",
    );
  });

  it("returns null when list empty", () => {
    expect(pickSelectedSkillKey([], "x", "y")).toBeNull();
  });
});

describe("sortAndFilterSkills", () => {
  it("sorts starred first within same status", () => {
    const ordered = sortAndFilterSkills(skills, "", ["1"]);
    expect(ordered.map((item) => item.skill_key)).toEqual([
      "zeta",
      "alpha",
      "old",
    ]);
  });

  it("keeps active ahead of archived even when archived is starred", () => {
    const ordered = sortAndFilterSkills(skills, "", ["3"]);
    expect(ordered.map((item) => item.skill_key)).toEqual([
      "alpha",
      "zeta",
      "old",
    ]);
  });

  it("sorts active before archived and global before project", () => {
    const ordered = sortAndFilterSkills(skills, "");
    expect(ordered.map((item) => item.skill_key)).toEqual([
      "alpha",
      "zeta",
      "old",
    ]);
  });

  it("sorts by name when status and scope are equal", () => {
    const equalScope = [
      {
        ...skills[0],
        id: "4",
        scope: "global",
        name: "Bravo",
        skill_key: "bravo",
      },
      {
        ...skills[1],
        id: "5",
        scope: "global",
        name: "Alpha",
        skill_key: "alpha-2",
      },
    ];

    const ordered = sortAndFilterSkills(equalScope, "");
    expect(ordered.map((item) => item.name)).toEqual(["Alpha", "Bravo"]);
  });

  it("sorts active before archived even in reverse order", () => {
    const reversed = [skills[2], skills[1]];
    const ordered = sortAndFilterSkills(reversed, "");
    expect(ordered.map((item) => item.status)).toEqual(["active", "archived"]);
  });

  it("sorts global before project even in reverse order", () => {
    const sameStatus = [skills[0], skills[1]];
    const ordered = sortAndFilterSkills(sameStatus, "");
    expect(ordered.map((item) => item.scope)).toEqual(["global", "project"]);
  });

  it("filters by key/name/scope/workspace", () => {
    expect(sortAndFilterSkills(skills, "alp")).toHaveLength(1);
    expect(sortAndFilterSkills(skills, "project")).toHaveLength(1);
    expect(sortAndFilterSkills(skills, "/tmp/repo")).toHaveLength(1);
    expect(sortAndFilterSkills(skills, "missing")).toHaveLength(0);
  });
});
