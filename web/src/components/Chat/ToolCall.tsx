import { useState } from "react";
import { Check, ChevronDown, ChevronRight, Copy, Hourglass, Loader2, XCircle } from "lucide-react";
import type { ChatMessage } from "../../types/events";

interface ToolCallProps {
  message: ChatMessage;
  onCopy?: () => void;
}

export function ToolCall({ message, onCopy }: ToolCallProps) {
  const [expanded, setExpanded] = useState(false);
  const tc = message.toolCall;
  if (!tc) return null;

  const status = tc.isStreaming ? "running" : tc.isError ? "error" : "completed";

  const statusIcon = {
    pending: <Hourglass className="w-3.5 h-3.5" />,
    running: <Loader2 className="w-3.5 h-3.5 animate-spin" />,
    completed: <Check className="w-3.5 h-3.5" />,
    error: <XCircle className="w-3.5 h-3.5" />,
  };

  const hasOutput = message.content || tc.output;

  return (
    <div className="flex justify-start message-item">
      <div className="max-w-[85%]">
        <div className="tool-call">
          <button
            className="tool-call-header w-full"
            onClick={() => setExpanded(!expanded)}
          >
            <div className="tool-call-header-left">
              <span className="tool-call-status" data-status={status}>
                {statusIcon[status]}
              </span>
              <span>{tc.name}</span>
            </div>
            <div className="flex items-center gap-1">
              {onCopy && (
                <span
                  className="message-action-button"
                  onClick={(e) => { e.stopPropagation(); onCopy(); }}
                  title="Copy output"
                >
                  <Copy className="w-3 h-3" />
                </span>
              )}
              <span className="text-neutral-500">
                {expanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
              </span>
            </div>
          </button>
          {expanded && hasOutput && (
            <div className="tool-call-body">
              <pre>{message.content || tc.output}</pre>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
