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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={() => {}}>
      <div className="bg-neutral-900 border border-neutral-700 rounded-xl shadow-2xl w-full max-w-md mx-4 overflow-hidden" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center gap-3 px-5 py-4 border-b border-neutral-700">
          <div className="text-amber-400">
            <ShieldAlert className="w-6 h-6" />
          </div>
          <div>
            <h2 className="text-base font-semibold text-neutral-100">
              {isPermissionHook ? "Permission Request" : "Approval Required"}
            </h2>
            <p className="text-xs text-neutral-400">
              {isPermissionHook ? `Tool "${payload.tool_name}" requires permission` : `Action: ${payload.tool_name}`}
            </p>
          </div>
        </div>

        <div className="px-5 py-4 space-y-3">
          {isPermissionHook && payload.permission && (
            <div className="flex items-center gap-2 text-sm text-neutral-300">
              {getToolIcon(payload.tool_name)}
              <span>
                Permission: <code className="px-1.5 py-0.5 rounded bg-neutral-800 text-amber-300 text-xs">{payload.permission}</code>
              </span>
            </div>
          )}
          <div className="bg-neutral-800 rounded-lg p-3 text-sm text-neutral-300 font-mono text-xs leading-relaxed max-h-32 overflow-y-auto">
            {payload.tool_args}
          </div>
        </div>

        <div className="flex gap-3 px-5 py-4 border-t border-neutral-700">
          <button
            onClick={() => onRespond(false)}
            className="flex-1 px-4 py-2 rounded-lg border border-neutral-600 text-neutral-300 hover:bg-neutral-800 hover:text-neutral-100 transition-colors text-sm font-medium"
          >
            Deny
          </button>
          <button
            onClick={() => onRespond(true)}
            className="flex-1 px-4 py-2 rounded-lg bg-amber-600 text-white hover:bg-amber-500 transition-colors text-sm font-medium"
          >
            Allow
          </button>
        </div>
      </div>
    </div>
  );
}
