import { Shield, ShieldAlert, Terminal, Globe, ListTodo, Bot } from "lucide-react";
import type { ApprovalRequiredPayload } from "../../types/events";

interface PermissionDialogProps {
  payload: ApprovalRequiredPayload;
  onRespond: (approved: boolean) => void;
}

function getToolIcon(toolName: string) {
  switch (toolName) {
    case "bash":
    case "Exec Policy":
      return <Terminal className="w-5 h-5" />;
    case "webfetch":
      return <Globe className="w-5 h-5" />;
    case "todo_plan":
      return <ListTodo className="w-5 h-5" />;
    case "subagent":
    case "Run project-local agents?":
      return <Bot className="w-5 h-5" />;
    default:
      return <Shield className="w-5 h-5" />;
  }
}

export function PermissionDialog({ payload, onRespond }: PermissionDialogProps) {
  const isPermissionHook = payload.source === "permission_hook";

  return (
    <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto">
        <div className="rounded-2xl border border-neutral-700 bg-neutral-900 overflow-hidden">
          <div className="flex items-center gap-2 px-4 py-3 border-b border-neutral-700">
            <div className="text-amber-400 shrink-0">
              <ShieldAlert className="w-5 h-5" />
            </div>
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium text-neutral-100 truncate">
                {isPermissionHook ? `Permission: ${payload.permission}` : payload.tool_name}
              </p>
              <p className="text-xs text-neutral-400 truncate">
                {isPermissionHook ? `Tool "${payload.tool_name}"` : payload.tool_args}
              </p>
            </div>
          </div>
          <div className="flex items-center justify-between px-4 py-2.5">
            <div className="flex items-center gap-2 text-xs text-neutral-400 min-w-0">
              {getToolIcon(payload.tool_name)}
              <span className="truncate">{payload.tool_args}</span>
            </div>
            <div className="flex items-center gap-2 shrink-0">
              <button
                onClick={() => onRespond(false)}
                className="px-3 py-1.5 rounded-lg border border-neutral-600 text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100 transition-colors text-xs font-medium"
              >
                Deny
              </button>
              <button
                onClick={() => onRespond(true)}
                className="px-3 py-1.5 rounded-lg bg-amber-600 text-white hover:bg-amber-500 transition-colors text-xs font-medium"
              >
                Allow
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
