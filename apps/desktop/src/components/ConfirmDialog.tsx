import * as AlertDialog from "@radix-ui/react-alert-dialog";
import type { ReactNode } from "react";
import { cn } from "../lib/cn";

type ConfirmDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  trigger: ReactNode;
  title: ReactNode;
  description: ReactNode;
  confirmLabel: string;
  pendingLabel?: string;
  cancelLabel?: string;
  pending?: boolean;
  error?: ReactNode;
  destructive?: boolean;
  onConfirm: () => void;
};

export function ConfirmDialog({
  open,
  onOpenChange,
  trigger,
  title,
  description,
  confirmLabel,
  pendingLabel = confirmLabel,
  cancelLabel = "Cancel",
  pending = false,
  error,
  destructive = false,
  onConfirm,
}: ConfirmDialogProps) {
  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(nextOpen) => {
        if (!pending) onOpenChange(nextOpen);
      }}
    >
      <AlertDialog.Trigger asChild>{trigger}</AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-50 bg-app-bg/80 backdrop-blur-[2px] data-[state=closed]:opacity-0 data-[state=open]:opacity-100 motion-safe:transition-opacity motion-safe:duration-150" />
        <AlertDialog.Content className="fixed top-1/2 left-1/2 z-50 w-[min(430px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 rounded-dialog border border-app-separator bg-app-surface p-6 text-app-text shadow-[0_24px_80px_rgb(0_0_0/.42)] outline-none data-[state=closed]:opacity-0 data-[state=open]:opacity-100 motion-safe:transition-opacity motion-safe:duration-150">
          <AlertDialog.Title className="m-0 text-lg font-bold tracking-[-.015em]">
            {title}
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 mb-0 text-xs/[19px] text-app-secondary">
            {description}
          </AlertDialog.Description>
          <div className="mt-6 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="h-9 rounded-control border border-app-separator bg-app-bg px-4 text-xs font-bold text-app-text hover:border-app-secondary disabled:opacity-50"
                disabled={pending}
              >
                {cancelLabel}
              </button>
            </AlertDialog.Cancel>
            <button
              type="button"
              className={cn(
                "h-9 rounded-control px-4 text-xs font-bold disabled:opacity-50",
                destructive
                  ? "bg-app-danger text-app-bg"
                  : "bg-app-accent text-app-on-accent",
              )}
              disabled={pending}
              onClick={onConfirm}
            >
              {pending ? pendingLabel : confirmLabel}
            </button>
          </div>
          {error ? (
            <p className="mt-3 mb-0 text-xs text-app-danger" role="alert">
              {error}
            </p>
          ) : null}
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
