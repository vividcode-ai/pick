interface ConfirmDialogProps {
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({ message, onConfirm, onCancel }: ConfirmDialogProps) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
      onClick={onCancel}
    >
      <div
        className="bg-neutral-900 border border-neutral-700 rounded-lg p-5 max-w-sm w-full mx-3 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <p className="text-sm text-neutral-200 mb-5">{message}</p>
        <div className="flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 text-sm rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={onConfirm}
            className="px-3 py-1.5 text-sm rounded-md bg-red-600 hover:bg-red-500 text-white transition-colors"
          >
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}
