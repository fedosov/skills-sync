import { useState } from "react";
import {
  listDotagentsMcp,
  listDotagentsSkills,
  migrateDotagents,
  runDotagentsSync,
} from "../tauriApi";

export type DotagentsProofStatus = "idle" | "running" | "ok" | "error";

const DOTAGENTS_MIGRATION_REQUIRED =
  "migration required before strict dotagents sync";

type RunAppAction = (
  action: () => Promise<void>,
  options?: {
    onError?: (message: string) => void | Promise<void>;
    skipIfBusy?: boolean;
  },
) => Promise<boolean>;

type UseDotagentsVerificationOptions = {
  runAppAction: RunAppAction;
  runtimeControls: { allow_filesystem_changes: boolean } | null;
  setError: (error: string) => void;
  refreshState: (options?: {
    preferredSkillKey?: string | null;
    withBusy?: boolean;
  }) => Promise<unknown>;
  selectedSkillKey: string | null;
  busy: boolean;
};

export function useDotagentsVerification({
  runAppAction,
  runtimeControls,
  setError,
  refreshState,
  selectedSkillKey,
  busy,
}: UseDotagentsVerificationOptions) {
  const [dotagentsProofStatus, setDotagentsProofStatus] =
    useState<DotagentsProofStatus>("idle");
  const [dotagentsProofSummary, setDotagentsProofSummary] = useState(
    "Dotagents check not run yet.",
  );
  const [dotagentsNeedsMigration, setDotagentsNeedsMigration] = useState(false);

  const FILESYSTEM_DISABLED_MESSAGE =
    "Filesystem changes are disabled. Enable 'Allow filesystem changes' first.";

  async function verifyDotagentsContracts() {
    await runDotagentsSync("all");
    const [skills, mcp] = await Promise.all([
      listDotagentsSkills("all"),
      listDotagentsMcp("all"),
    ]);
    setDotagentsProofStatus("ok");
    setDotagentsProofSummary(
      `Dotagents verified: skills=${skills.length}, mcp=${mcp.length}.`,
    );
    await refreshState({
      preferredSkillKey: selectedSkillKey,
      withBusy: false,
    });
  }

  function applyDotagentsVerificationError(message: string) {
    const migrationRequired = message
      .toLowerCase()
      .includes(DOTAGENTS_MIGRATION_REQUIRED);
    setDotagentsNeedsMigration(migrationRequired);
    setDotagentsProofStatus("error");
    setDotagentsProofSummary(
      migrationRequired
        ? "Dotagents contracts are missing. Run Initialize dotagents."
        : `Dotagents check failed: ${message}`,
    );
    setError(message);
  }

  async function handleVerifyDotagents() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }

    setDotagentsNeedsMigration(false);
    setDotagentsProofStatus("running");
    setDotagentsProofSummary("Verifying dotagents commands...");
    await runAppAction(verifyDotagentsContracts, {
      onError: applyDotagentsVerificationError,
      skipIfBusy: true,
    });
  }

  async function handleInitializeDotagents() {
    if (!runtimeControls?.allow_filesystem_changes) {
      setError(FILESYSTEM_DISABLED_MESSAGE);
      return;
    }

    setDotagentsNeedsMigration(false);
    setDotagentsProofStatus("running");
    setDotagentsProofSummary("Initializing dotagents contracts...");
    await runAppAction(
      async () => {
        await migrateDotagents("all");
        setDotagentsProofSummary("Verifying dotagents commands...");
        await verifyDotagentsContracts();
      },
      {
        onError: (message) => {
          setDotagentsProofStatus("error");
          setDotagentsProofSummary(
            `Dotagents initialization failed: ${message}`,
          );
          setError(message);
        },
        skipIfBusy: true,
      },
    );
  }

  return {
    dotagentsProofStatus,
    dotagentsProofSummary,
    dotagentsNeedsMigration,
    handleVerifyDotagents,
    handleInitializeDotagents,
    busy,
  };
}
