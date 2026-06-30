import { useState } from "react";
import type { ChatMessage } from "../../types/events";

interface MessageBubbleProps {
  message: ChatMessage;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  switch (message.role) {
    case "user":
      return <UserBubble message={message} />;
    case "assistant":
      return <AssistantBubble message={message} />;
    case "thinking":
      return <ThinkingBubble message={message} />;
    case "tool":
      return <ToolBubble message={message} />;
    case "system":
      return <SystemBubble message={message} />;
    default:
      return null;
  }
}

function UserBubble({ message }: { message: ChatMessage }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[75%] rounded-2xl rounded-br-md bg-blue-600 text-white px-4 py-2.5 whitespace-pre-wrap text-sm leading-relaxed">
        {message.content}
      </div>
    </div>
  );
}

function AssistantBubble({ message }: { message: ChatMessage }) {
  return (
    <div className="flex justify-start">
      <div className="max-w-[75%] rounded-2xl rounded-bl-md bg-neutral-800 text-neutral-100 px-4 py-2.5 whitespace-pre-wrap text-sm leading-relaxed">
        {message.content}
      </div>
    </div>
  );
}

function ThinkingBubble({ message }: { message: ChatMessage }) {
  const [open, setOpen] = useState(false);

  return (
    <div className="flex justify-start">
      <div className="max-w-[75%]">
        <button
          onClick={() => setOpen(!open)}
          className="flex items-center gap-2 w-full px-3 py-1.5 rounded-lg bg-neutral-900 border border-neutral-700 text-neutral-400 hover:text-neutral-300 text-xs transition-colors"
        >
          <svg
            className={`w-3.5 h-3.5 transition-transform ${open ? "rotate-90" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M9 5l7 7-7 7"
            />
          </svg>
          <span className="font-medium">Thinking</span>
        </button>
        {open && (
          <div className="mt-1 px-3 py-2 rounded-lg bg-neutral-900 border border-neutral-700 text-neutral-400 text-xs whitespace-pre-wrap leading-relaxed">
            {message.content}
          </div>
        )}
      </div>
    </div>
  );
}

function ToolBubble({ message }: { message: ChatMessage }) {
  const tc = message.toolCall;
  if (!tc) return null;

  return (
    <div className="flex justify-start">
      <div className="max-w-[75%] border border-neutral-700 rounded-lg overflow-hidden">
        <div className="flex items-center gap-2 px-3 py-2 bg-neutral-800 text-sm text-neutral-300">
          <span className="font-mono text-xs">{tc.name}</span>
          {tc.isStreaming && (
            <span className="w-2 h-2 bg-yellow-400 rounded-full animate-pulse" />
          )}
          {tc.isError && <span className="text-red-400 text-xs">Error</span>}
        </div>
        {tc.isStreaming && message.content && (
          <pre className="px-3 py-2 text-sm text-neutral-300 overflow-x-auto">
            {message.content}
          </pre>
        )}
        {tc.output && !tc.isStreaming && (
          <pre className="px-3 py-2 text-sm text-neutral-300 overflow-x-auto max-h-60">
            {tc.output}
          </pre>
        )}
      </div>
    </div>
  );
}

function SystemBubble({ message }: { message: ChatMessage }) {
  return (
    <div className="flex justify-center">
      <div className="text-xs text-neutral-500 bg-neutral-900 rounded-full px-3 py-1">
        {message.content}
      </div>
    </div>
  );
}
