import { Button } from "../ui/button";

export function RemovalActions({
  isRemoving,
  busyAction,
  extraDisabled = false,
  onToggle,
  onCancel,
  onConfirm,
}: {
  isRemoving: boolean;
  busyAction: string | null;
  extraDisabled?: boolean;
  onToggle: () => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  if (isRemoving) {
    return (
      <>
        <Button
          size="sm"
          variant="outline"
          onClick={onCancel}
          disabled={busyAction !== null}
        >
          Cancel
        </Button>
        <Button
          size="sm"
          variant="destructive"
          onClick={onConfirm}
          disabled={busyAction !== null}
        >
          Confirm remove
        </Button>
      </>
    );
  }
  return (
    <Button
      size="sm"
      variant="ghost"
      className="text-muted-foreground/60 hover:text-destructive"
      onClick={onToggle}
      disabled={busyAction !== null || extraDisabled}
    >
      Remove
    </Button>
  );
}
