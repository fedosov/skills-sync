import { sortAndFilter, statusRank } from "./lib/catalogUtils";
import { pickPreferred } from "./lib/utils";
import type { SkillRecord } from "./types";

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

export function pickSelectedSkillKey(
  skills: SkillRecord[],
  preferredKey?: string | null,
  previousKey?: string | null,
): string | null {
  return pickPreferred(skills, preferredKey, previousKey, (s) => s.skill_key);
}

function skillComparator(starredSkillIds: string[]) {
  const starred = new Set(starredSkillIds);
  const starredRank = (id: string) => (starred.has(id) ? 0 : 1);
  const scopeRank = (scope: SkillRecord["scope"]) =>
    scope === "global" ? 0 : 1;

  return (lhs: SkillRecord, rhs: SkillRecord) =>
    statusRank(lhs.status) - statusRank(rhs.status) ||
    starredRank(lhs.id) - starredRank(rhs.id) ||
    scopeRank(lhs.scope) - scopeRank(rhs.scope) ||
    lhs.name.localeCompare(rhs.name);
}

const skillSearchFields = (skill: SkillRecord) => [
  skill.name,
  skill.skill_key,
  skill.scope,
  skill.workspace ?? "",
];

export function sortAndFilterSkills(
  skills: SkillRecord[],
  query: string,
  starredSkillIds: string[] = [],
): SkillRecord[] {
  return sortAndFilter(
    skills,
    query,
    skillComparator(starredSkillIds),
    skillSearchFields,
  );
}
