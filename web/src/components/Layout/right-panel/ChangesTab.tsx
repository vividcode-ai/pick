import type { CSSProperties } from "react";

interface ChangesTabProps {
  diffs?: { filePath: string; content: string }[];
}

export function ChangesTab({ diffs }: ChangesTabProps) {
  if (!diffs || diffs.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-neutral-500">
        No changes yet
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1 p-2">
      {diffs.map((diff, i) => (
        <div key={i} className="text-sm text-neutral-300 px-2 py-1 rounded hover:bg-neutral-800 cursor-pointer">
          {diff.filePath}
        </div>
      ))}
    </div>
  );
}
