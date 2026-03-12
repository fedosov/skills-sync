import type { CatalogMutationRequest, FocusKind } from "../types";

export type DotagentsProofStatus = "idle" | "running" | "ok" | "error";

export type DeleteDialogState = {
  request: CatalogMutationRequest | null;
  label: string;
  onConfirmOverride?: () => Promise<void>;
} | null;

export type OpenTargetMenu = "skill" | "subagent" | null;

export type ActionsMenuTarget = "skill" | "subagent" | "mcp" | null;

export type CatalogProjectGroupState = Record<
  FocusKind,
  Record<string, boolean | undefined>
>;
