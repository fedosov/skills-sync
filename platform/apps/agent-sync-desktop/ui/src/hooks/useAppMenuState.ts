import { useEffect, useState } from "react";
import type {
  ActionsMenuTarget,
  DeleteDialogState,
  OpenTargetMenu,
} from "../lib/uiStateTypes";

export function useAppMenuState() {
  const [openTargetMenu, setOpenTargetMenu] = useState<OpenTargetMenu>(null);
  const [actionsMenuTarget, setActionsMenuTarget] =
    useState<ActionsMenuTarget>(null);
  const [deleteDialog, setDeleteDialog] = useState<DeleteDialogState>(null);
  const [auditOpen, setAuditOpen] = useState(false);

  function closeMenus() {
    setActionsMenuTarget(null);
    setOpenTargetMenu(null);
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      setOpenTargetMenu(null);
      setActionsMenuTarget(null);
      setDeleteDialog(null);
      setAuditOpen(false);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  return {
    openTargetMenu,
    setOpenTargetMenu,
    actionsMenuTarget,
    setActionsMenuTarget,
    deleteDialog,
    setDeleteDialog,
    auditOpen,
    setAuditOpen,
    closeMenus,
  };
}
