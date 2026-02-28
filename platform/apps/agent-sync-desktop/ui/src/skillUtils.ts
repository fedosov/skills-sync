import type { SkillRecord } from "./types";

export type { SkillRecord };

export function normalizeSkillKey(title: string): string {
  const trimmed = title.trim().toLowerCase();
  if (!trimmed) return "";

  let normalized = "";
  let previousDash = false;
  for (const char of trimmed) {
    const isAlphaNum =
      (char >= "a" && char <= "z") || (char >= "0" && char <= "9");
    if (isAlphaNum) {
      normalized += char;
      previousDash = false;
      continue;
    }
    if (!previousDash) {
      normalized += "-";
      previousDash = true;
    }
  }

  return normalized.replace(/^-+|-+$/g, "");
}

export function formatUnixTime(value: number | null): string {
  if (value == null || Number.isNaN(value)) return "-";
  const date = new Date(value * 1000);
  if (Number.isNaN(date.getTime())) return "-";
  return date.toLocaleString();
}

export function pickSelectedSkillKey(
  skills: SkillRecord[],
  preferredKey?: string | null,
  previousKey?: string | null,
): string | null {
  if (
    preferredKey &&
    skills.some((skill) => skill.skill_key === preferredKey)
  ) {
    return preferredKey;
  }
  if (previousKey && skills.some((skill) => skill.skill_key === previousKey)) {
    return previousKey;
  }
  return skills[0]?.skill_key ?? null;
}

export function sortAndFilterSkills(
  skills: SkillRecord[],
  query: string,
  starredSkillIds: string[] = [],
): SkillRecord[] {
  const normalizedQuery = query.trim().toLowerCase();
  const starred = new Set(starredSkillIds);
  const statusRank = (status: SkillRecord["status"]) =>
    status === "active" ? 0 : 1;
  const starredRank = (id: string) => (starred.has(id) ? 0 : 1);
  const scopeRank = (scope: SkillRecord["scope"]) =>
    scope === "global" ? 0 : 1;

  const ordered = skills.slice().sort((lhs, rhs) => {
    const byStatus = statusRank(lhs.status) - statusRank(rhs.status);
    if (byStatus !== 0) {
      return byStatus;
    }
    const byStarred = starredRank(lhs.id) - starredRank(rhs.id);
    if (byStarred !== 0) {
      return byStarred;
    }
    const byScope = scopeRank(lhs.scope) - scopeRank(rhs.scope);
    if (byScope !== 0) {
      return byScope;
    }
    return lhs.name.localeCompare(rhs.name);
  });

  if (!normalizedQuery) return ordered;

  return ordered.filter((skill) => {
    return (
      skill.name.toLowerCase().includes(normalizedQuery) ||
      skill.skill_key.toLowerCase().includes(normalizedQuery) ||
      skill.scope.toLowerCase().includes(normalizedQuery) ||
      (skill.workspace ?? "").toLowerCase().includes(normalizedQuery)
    );
  });
}
