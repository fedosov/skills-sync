import { Button } from "./ui/button";
import type { DeleteDialogState } from "../hooks/useAppMenuState";
import type { CatalogMutationRequest } from "../types";

type DeleteConfirmDialogProps = {
  deleteDialog: DeleteDialogState;
  busy: boolean;
  onClose: () => void;
  onConfirm: (request: CatalogMutationRequest) => void;
};

export function DeleteConfirmDialog({
  deleteDialog,
  busy,
  onClose,
  onConfirm,
}: DeleteConfirmDialogProps) {
  if (!deleteDialog) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Confirm delete"
        className="w-full max-w-sm rounded-md border border-border/70 bg-card p-4"
      >
        <h2 className="text-sm font-semibold">Confirm delete</h2>
        <p className="mt-2 text-xs text-muted-foreground">
          Remove {deleteDialog.label}? This action moves files to system Trash.
        </p>
        <div className="mt-3 flex items-center justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            size="sm"
            variant="destructive"
            disabled={busy}
            onClick={() => {
              const { request, onConfirmOverride } = deleteDialog;
              onClose();
              if (onConfirmOverride) {
                void onConfirmOverride();
              } else if (request) {
                onConfirm(request);
              }
            }}
          >
            Delete
          </Button>
        </div>
      </div>
    </div>
  );
}
