import { AgentsDetailsPanel } from "../details/AgentsDetailsPanel";
import { McpDetailsPanel } from "../details/McpDetailsPanel";
import { SkillDetailsPanel } from "../details/SkillDetailsPanel";
import { SubagentDetailsPanel } from "../details/SubagentDetailsPanel";
import { Card, CardContent } from "../ui/card";
import type { ActionsMenuTarget, OpenTargetMenu } from "../../lib/uiStateTypes";
import type {
  AgentContextEntry,
  AgentContextSegment,
  McpServerRecord,
  SkillDetails,
  SubagentDetails,
} from "../../types";

export type DetailsPaneProps = {
  busy: boolean;
  skillDetails: SkillDetails | null;
  skillStarred: boolean;
  renameDraft: string;
  subagentDetails: SubagentDetails | null;
  subagentFavorite: boolean;
  selectedMcpServer: McpServerRecord | null;
  selectedMcpWarnings: string[];
  selectedMcpKey: string | null;
  mcpFavorite: boolean;
  selectedAgentEntry: AgentContextEntry | null;
  selectedAgentTopSegments: AgentContextSegment[];
  agentFavorite: boolean;
  actionsMenuTarget: ActionsMenuTarget;
  openTargetMenu: OpenTargetMenu;
  fixingSyncWarning: string | null;
  onToggleSkillStar: () => void;
  onRenameDraftChange: (value: string) => void;
  onSkillRenameSubmit: () => void;
  onToggleSkillOpenTargetMenu: () => void;
  onToggleSkillActionsMenu: () => void;
  onSkillOpenPath: (target: "folder" | "file") => void;
  onSkillArchive: () => void;
  onSkillMakeGlobal: () => void;
  onSkillRestore: () => void;
  onSkillRequestDelete: () => void;
  onSkillCopyPath: (path: string, errorLabel: string) => void;
  onToggleMcpFavorite: () => void;
  onToggleMcpActionsMenu: () => void;
  onMcpSetEnabled: (agent: "codex" | "claude", enabled: boolean) => void;
  onMcpFixWarning: (warning: string) => void;
  onMcpArchive: () => void;
  onMcpMakeGlobal: () => void;
  onMcpRestore: () => void;
  onMcpRequestDelete: () => void;
  onToggleAgentFavorite: () => void;
  onToggleSubagentFavorite: () => void;
  onToggleSubagentOpenTargetMenu: () => void;
  onToggleSubagentActionsMenu: () => void;
  onSubagentOpenPath: (target: "folder" | "file") => void;
  onSubagentArchive: () => void;
  onSubagentRestore: () => void;
  onSubagentRequestDelete: () => void;
};

export function DetailsPane({
  busy,
  skillDetails,
  skillStarred,
  renameDraft,
  subagentDetails,
  subagentFavorite,
  selectedMcpServer,
  selectedMcpWarnings,
  selectedMcpKey,
  mcpFavorite,
  selectedAgentEntry,
  selectedAgentTopSegments,
  agentFavorite,
  actionsMenuTarget,
  openTargetMenu,
  fixingSyncWarning,
  onToggleSkillStar,
  onRenameDraftChange,
  onSkillRenameSubmit,
  onToggleSkillOpenTargetMenu,
  onToggleSkillActionsMenu,
  onSkillOpenPath,
  onSkillArchive,
  onSkillMakeGlobal,
  onSkillRestore,
  onSkillRequestDelete,
  onSkillCopyPath,
  onToggleMcpFavorite,
  onToggleMcpActionsMenu,
  onMcpSetEnabled,
  onMcpFixWarning,
  onMcpArchive,
  onMcpMakeGlobal,
  onMcpRestore,
  onMcpRequestDelete,
  onToggleAgentFavorite,
  onToggleSubagentFavorite,
  onToggleSubagentOpenTargetMenu,
  onToggleSubagentActionsMenu,
  onSubagentOpenPath,
  onSubagentArchive,
  onSubagentRestore,
  onSubagentRequestDelete,
}: DetailsPaneProps) {
  const showSkill = !!skillDetails;
  const showSubagent = !!subagentDetails;
  const showMcp = !!selectedMcpServer;
  const showAgents = !!selectedAgentEntry;

  return (
    <Card className="min-h-[520px] overflow-hidden lg:flex lg:h-full lg:min-h-0 lg:flex-col">
      {!showSkill && !showSubagent && !showMcp && !showAgents ? (
        <CardContent className="flex h-full items-center justify-center text-sm text-muted-foreground lg:min-h-0 lg:flex-1">
          Select an item to view details.
        </CardContent>
      ) : null}

      {skillDetails ? (
        <SkillDetailsPanel
          details={skillDetails}
          busy={busy}
          isFavorite={skillStarred}
          onToggleFavorite={onToggleSkillStar}
          renameDraft={renameDraft}
          openTargetMenu={openTargetMenu === "skill"}
          actionsMenuOpen={actionsMenuTarget === "skill"}
          onRenameDraftChange={onRenameDraftChange}
          onRenameSubmit={onSkillRenameSubmit}
          onToggleOpenTargetMenu={onToggleSkillOpenTargetMenu}
          onToggleActionsMenu={onToggleSkillActionsMenu}
          onOpenPath={onSkillOpenPath}
          onArchive={onSkillArchive}
          onMakeGlobal={onSkillMakeGlobal}
          onRestore={onSkillRestore}
          onRequestDelete={onSkillRequestDelete}
          onCopyPath={onSkillCopyPath}
        />
      ) : null}

      {selectedMcpServer ? (
        <McpDetailsPanel
          server={selectedMcpServer}
          warnings={selectedMcpWarnings}
          busy={busy}
          fixingWarning={fixingSyncWarning}
          isFavorite={selectedMcpKey ? mcpFavorite : false}
          onToggleFavorite={onToggleMcpFavorite}
          actionsMenuOpen={actionsMenuTarget === "mcp"}
          onToggleActionsMenu={onToggleMcpActionsMenu}
          onSetEnabled={onMcpSetEnabled}
          onFixWarning={onMcpFixWarning}
          onArchive={onMcpArchive}
          onMakeGlobal={onMcpMakeGlobal}
          onRestore={onMcpRestore}
          onRequestDelete={onMcpRequestDelete}
        />
      ) : null}

      {selectedAgentEntry ? (
        <AgentsDetailsPanel
          entry={selectedAgentEntry}
          topSegments={selectedAgentTopSegments}
          isFavorite={agentFavorite}
          onToggleFavorite={onToggleAgentFavorite}
        />
      ) : null}

      {subagentDetails ? (
        <SubagentDetailsPanel
          subagentDetails={subagentDetails}
          busy={busy}
          isFavorite={subagentFavorite}
          onToggleFavorite={onToggleSubagentFavorite}
          openTargetMenu={openTargetMenu === "subagent"}
          actionsMenuOpen={actionsMenuTarget === "subagent"}
          onToggleOpenTargetMenu={onToggleSubagentOpenTargetMenu}
          onToggleActionsMenu={onToggleSubagentActionsMenu}
          onOpenPath={onSubagentOpenPath}
          onArchive={onSubagentArchive}
          onRestore={onSubagentRestore}
          onRequestDelete={onSubagentRequestDelete}
        />
      ) : null}
    </Card>
  );
}
