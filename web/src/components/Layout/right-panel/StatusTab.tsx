import { type ReactNode } from "react";

interface StatusTabProps {
  connected: boolean;
  connectionStatus?: ReactNode;
}

export function StatusTab({ connected }: StatusTabProps) {
  return (
    <div className="p-3 space-y-3 text-sm">
      <div className="flex items-center justify-between">
        <span className="text-neutral-400">Connection</span>
        <span className={`flex items-center gap-1.5 ${connected ? "text-green-400" : "text-red-400"}`}>
          <span className={`w-2 h-2 rounded-full ${connected ? "bg-green-500" : "bg-red-500"}`} />
          {connected ? "Connected" : "Disconnected"}
        </span>
      </div>
    </div>
  );
}
