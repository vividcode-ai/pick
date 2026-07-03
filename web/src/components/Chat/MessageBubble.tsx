import { useState, useCallback } from "react";
import { Copy, GitFork, ChevronRight, ChevronDown, Undo, Trash2 } from "lucide-react";
import type { ChatMessage } from "../../types/events";
import { Markdown } from "./Markdown";
import { ToolCall } from "./ToolCall";


interface MessageBubbleProps {
  message: ChatMessage;
  onFork?: () => void;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

export function MessageBubble({ message, onFork }: MessageBubbleProps) {
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content);
  }, [message.content]);

  switch (message.role) {
    case "user":
      return <UserBubble message={message} onCopy={handleCopy} />;
    case "assistant":
      return <AssistantBubble message={message} onCopy={handleCopy} onFork={onFork} />;
    case "thinking":
      return <ThinkingBubble message={message} />;
    case "tool":
      return <ToolCall message={message} onCopy={handleCopy} />;
    case "system":
      return <SystemBubble message={message} />;
    default:
      return null;
  }
}

function UserBubble({ message, onCopy }: { message: ChatMessage; onCopy: () => void }) {
  return (
    <div className="flex justify-end message-item">
      <div className="user-message-bubble">
        <div className="text-sm leading-relaxed whitespace-pre-wrap text-[var(--text-primary)]">
          {message.content}
        </div>
      </div>
      <div className="message-actions justify-end pr-1">
        <button className="message-action-button" onClick={onCopy} title="Copy">
          <Copy className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}

function AssistantBubble({ message, onCopy, onFork }: { message: ChatMessage; onCopy: () => void; onFork?: () => void }) {
  return (
    <div className="flex justify-start message-item">
      <div className="w-full">
        <div className="assistant-content-bubble">
          <Markdown content={message.content} />
        </div>
        <div className="message-actions pl-1 pt-0.5">
          <button className="message-action-button" onClick={onFork} title="Fork">
            <GitFork className="w-3 h-3" />
          </button>
          <button className="message-action-button" onClick={onCopy} title="Copy">
            <Copy className="w-3 h-3" />
          </button>
        </div>
      </div>
    </div>
  );
}

function ThinkingBubble({ message }: { message: ChatMessage }) {
  const [open, setOpen] = useState(false);

  return (
    <div className="flex justify-start message-item">
      <div className="w-full">
        <button
          onClick={() => setOpen(!open)}
          className="reasoning-toggle w-full"
        >
          {open ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          <span className="font-medium">Thinking</span>
          <span className="text-neutral-500">·</span>
          <span className="text-neutral-500">{formatTime(message.timestamp)}</span>
        </button>
        {open && (
          <div className="reasoning-body">
            <pre className="reasoning-text">{message.content}</pre>
          </div>
        )}
      </div>
    </div>
  );
}

function SystemBubble({ message }: { message: ChatMessage }) {
  return (
    <div className="flex justify-center message-item">
      <div className="text-xs text-neutral-500 bg-neutral-800 rounded-full px-3 py-1">
        {message.content}
      </div>
    </div>
  );
}
