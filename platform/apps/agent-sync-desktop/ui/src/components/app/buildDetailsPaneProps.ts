import { mcpDeleteLabel, mcpStatus, mcpTarget } from "../../lib/catalogUtils";
import type {
  ActionsMenuTarget,
  DeleteDialogState,
  OpenTargetMenu,
} from "../../lib/uiStateTypes";
import type {
  AgentContextEntry,
  AgentContextSegment,
  CatalogMutationRequest,
  FocusKind,
  McpServerRecord,
  SkillDetails,
  SubagentDetails,
} from "../../types";
import type { DetailsPaneProps } from "./DetailsPane";

type BuildDetailsPanePropsOptions = {
  focusKind: FocusKind;
  busy: boolean;
  details: SkillDetails | null;
  renameDraft: string;
  subagentDetails: SubagentDetails | null;
  selectedMcpServer: McpServerRecord | null;
  selectedMcpWarnings: string[];
  selectedMcpKey: string | null;
  selectedAgentEntry: AgentContextEntry | null;
  selectedAgentTopSegments: AgentContextSegment[];
  actionsMenuTarget: ActionsMenuTarget;
  openTargetMenu: OpenTargetMenu;
  fixingSyncWarning: string | null;
  starredSkillSet: Set<string>;
  favorites: {
    subagents: Set<string>;
    mcp: Set<string>;
    agents: Set<string>;
  };
  toggleFavorite: (kind: "subagents" | "mcp" | "agents", id: string) => void;
  setOpenTargetMenu: (
    value: OpenTargetMenu | ((prev: OpenTargetMenu) => OpenTargetMenu),
  ) => void;
  setActionsMenuTarget: (
    value: ActionsMenuTarget | ((prev: ActionsMenuTarget) => ActionsMenuTarget),
  ) => void;
  setDeleteDialog: (
    value: DeleteDialogState | ((prev: DeleteDialogState) => DeleteDialogState),
  ) => void;
  setRenameDraft: (value: string) => void;
  handleToggleSkillStar: (skillId: string) => Promise<void>;
  handleRenameSkill: (skillKey: string, rawTitle: string) => Promise<void>;
  handleOpenSkillPath: (
    skillKey: string,
    target: "folder" | "file",
  ) => Promise<void>;
  handleOpenSubagentPath: (
    subagentId: string,
    target: "folder" | "file",
  ) => Promise<void>;
  handleSetMcpEnabled: (
    server: McpServerRecord,
    agent: "codex" | "claude",
    enabled: boolean,
  ) => Promise<void>;
  handleDeleteUnmanagedMcp: (serverKey: string) => Promise<void>;
  handleFixSyncWarning: (warning: string) => Promise<void>;
  executeCatalogMutation: (
    request: CatalogMutationRequest,
    preferredSkillKey?: string | null,
  ) => Promise<void>;
  copyPath: (path: string, errorLabel: string) => Promise<void>;
};

export function buildDetailsPaneProps({
  focusKind,
  busy,
  details,
  renameDraft,
  subagentDetails,
  selectedMcpServer,
  selectedMcpWarnings,
  selectedMcpKey,
  selectedAgentEntry,
  selectedAgentTopSegments,
  actionsMenuTarget,
  openTargetMenu,
  fixingSyncWarning,
  starredSkillSet,
  favorites,
  toggleFavorite,
  setOpenTargetMenu,
  setActionsMenuTarget,
  setDeleteDialog,
  setRenameDraft,
  handleToggleSkillStar,
  handleRenameSkill,
  handleOpenSkillPath,
  handleOpenSubagentPath,
  handleSetMcpEnabled,
  handleDeleteUnmanagedMcp,
  handleFixSyncWarning,
  executeCatalogMutation,
  copyPath,
}: BuildDetailsPanePropsOptions): DetailsPaneProps {
  return {
    busy,
    skillDetails: focusKind === "skills" ? details : null,
    skillStarred: !!details && starredSkillSet.has(details.skill.id),
    renameDraft,
    subagentDetails: focusKind === "subagents" ? subagentDetails : null,
    subagentFavorite:
      !!subagentDetails && favorites.subagents.has(subagentDetails.subagent.id),
    selectedMcpServer: focusKind === "mcp" ? selectedMcpServer : null,
    selectedMcpWarnings,
    selectedMcpKey,
    mcpFavorite: !!selectedMcpKey && favorites.mcp.has(selectedMcpKey),
    selectedAgentEntry: focusKind === "agents" ? selectedAgentEntry : null,
    selectedAgentTopSegments,
    agentFavorite:
      !!selectedAgentEntry && favorites.agents.has(selectedAgentEntry.id),
    actionsMenuTarget,
    openTargetMenu,
    fixingSyncWarning,
    onToggleSkillStar: () => {
      if (details) {
        void handleToggleSkillStar(details.skill.id);
      }
    },
    onRenameDraftChange: setRenameDraft,
    onSkillRenameSubmit: () => {
      if (details) {
        void handleRenameSkill(details.skill.skill_key, renameDraft);
      }
    },
    onToggleSkillOpenTargetMenu: () => {
      setOpenTargetMenu((prev) => (prev === "skill" ? null : "skill"));
      setActionsMenuTarget(null);
    },
    onToggleSkillActionsMenu: () => {
      setActionsMenuTarget((prev) => (prev === "skill" ? null : "skill"));
      setOpenTargetMenu(null);
    },
    onSkillOpenPath: (target) => {
      if (details) {
        void handleOpenSkillPath(details.skill.skill_key, target);
      }
    },
    onSkillArchive: () => {
      if (!details) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation(
        {
          action: "archive",
          target: {
            kind: "skill",
            skillKey: details.skill.skill_key,
          },
          confirmed: true,
        },
        details.skill.skill_key,
      );
    },
    onSkillMakeGlobal: () => {
      if (!details) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation(
        {
          action: "make_global",
          target: {
            kind: "skill",
            skillKey: details.skill.skill_key,
          },
          confirmed: true,
        },
        details.skill.skill_key,
      );
    },
    onSkillRestore: () => {
      if (!details) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation(
        {
          action: "restore",
          target: {
            kind: "skill",
            skillKey: details.skill.skill_key,
          },
          confirmed: true,
        },
        details.skill.skill_key,
      );
    },
    onSkillRequestDelete: () => {
      if (!details) return;
      setActionsMenuTarget(null);
      setDeleteDialog({
        request: {
          action: "delete",
          target: {
            kind: "skill",
            skillKey: details.skill.skill_key,
          },
          confirmed: true,
        },
        label: `skill "${details.skill.name}"`,
      });
    },
    onSkillCopyPath: (path, errorLabel) => void copyPath(path, errorLabel),
    onToggleMcpFavorite: () => {
      if (selectedMcpKey) {
        toggleFavorite("mcp", selectedMcpKey);
      }
    },
    onToggleMcpActionsMenu: () => {
      setActionsMenuTarget((prev) => (prev === "mcp" ? null : "mcp"));
      setOpenTargetMenu(null);
    },
    onMcpSetEnabled: (agent, enabled) => {
      if (selectedMcpServer) {
        void handleSetMcpEnabled(selectedMcpServer, agent, enabled);
      }
    },
    onMcpFixWarning: (warning) => void handleFixSyncWarning(warning),
    onMcpArchive: () => {
      if (!selectedMcpServer) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation({
        action: "archive",
        target: mcpTarget(selectedMcpServer),
        confirmed: true,
      });
    },
    onMcpMakeGlobal: () => {
      if (!selectedMcpServer) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation({
        action: "make_global",
        target: mcpTarget(selectedMcpServer),
        confirmed: true,
      });
    },
    onMcpRestore: () => {
      if (!selectedMcpServer) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation({
        action: "restore",
        target: mcpTarget(selectedMcpServer),
        confirmed: true,
      });
    },
    onMcpRequestDelete: () => {
      if (!selectedMcpServer) return;
      setActionsMenuTarget(null);
      if (mcpStatus(selectedMcpServer) === "unmanaged") {
        const serverKey = selectedMcpServer.server_key;
        setDeleteDialog({
          request: null,
          label: mcpDeleteLabel(selectedMcpServer),
          onConfirmOverride: async () => handleDeleteUnmanagedMcp(serverKey),
        });
      } else {
        setDeleteDialog({
          request: {
            action: "delete",
            target: mcpTarget(selectedMcpServer),
            confirmed: true,
          },
          label: mcpDeleteLabel(selectedMcpServer),
        });
      }
    },
    onToggleAgentFavorite: () => {
      if (selectedAgentEntry) {
        toggleFavorite("agents", selectedAgentEntry.id);
      }
    },
    onToggleSubagentFavorite: () => {
      if (subagentDetails) {
        toggleFavorite("subagents", subagentDetails.subagent.id);
      }
    },
    onToggleSubagentOpenTargetMenu: () => {
      setOpenTargetMenu((prev) => (prev === "subagent" ? null : "subagent"));
      setActionsMenuTarget(null);
    },
    onToggleSubagentActionsMenu: () => {
      setActionsMenuTarget((prev) => (prev === "subagent" ? null : "subagent"));
      setOpenTargetMenu(null);
    },
    onSubagentOpenPath: (target) => {
      if (subagentDetails) {
        void handleOpenSubagentPath(subagentDetails.subagent.id, target);
      }
    },
    onSubagentArchive: () => {
      if (!subagentDetails) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation({
        action: "archive",
        target: {
          kind: "subagent",
          subagentId: subagentDetails.subagent.id,
        },
        confirmed: true,
      });
    },
    onSubagentRestore: () => {
      if (!subagentDetails) return;
      setActionsMenuTarget(null);
      void executeCatalogMutation({
        action: "restore",
        target: {
          kind: "subagent",
          subagentId: subagentDetails.subagent.id,
        },
        confirmed: true,
      });
    },
    onSubagentRequestDelete: () => {
      if (!subagentDetails) return;
      setActionsMenuTarget(null);
      setDeleteDialog({
        request: {
          action: "delete",
          target: {
            kind: "subagent",
            subagentId: subagentDetails.subagent.id,
          },
          confirmed: true,
        },
        label: `subagent "${subagentDetails.subagent.name}"`,
      });
    },
  };
}
