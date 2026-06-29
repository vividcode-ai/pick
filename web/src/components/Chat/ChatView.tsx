import { useRef, useEffect } from "react";
import type { ChatMessage } from "../../types/events";

interface ChatViewProps {
  messages: ChatMessage[];
  streaming: boolean;
}

export function ChatView({ messages, streaming }: ChatViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  return (
    <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
      {messages.length === 0 && (
        <div className="flex items-center justify-center h-full text-neutral-500">
          <p className="text-lg">Send a message to start</p>
        </div>
      )}
      {messages.map((msg) => (
        <MessageBubble key={msg.id} message={msg} />
      ))}
      {streaming && (
        <div className="flex items-center gap-2 text-neutral-400 px-4 py-2">
          <span className="w-2 h-2 bg-neutral-400 rounded-full animate-pulse" />
          <span className="text-sm">Thinking...</span>
        </div>
      )}
      <div ref={bottomRef} />
    </div>
  );
}

function MessageBubble({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";
  const isTool = message.role === "tool";
  const isSystem = message.role === "system";

  if (isTool && message.toolCall) {
    return <ToolCallCard message={message} />;
  }

  if (isSystem) {
    return (
      <div className="flex justify-center">
        <div className="text-xs text-neutral-500 bg-neutral-900 rounded-full px-3 py-1">
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[80%] rounded-lg px-4 py-2 whitespace-pre-wrap ${
          isUser
            ? "bg-blue-600 text-white"
            : "bg-neutral-800 text-neutral-100"
        }`}
      >
        {message.content}
      </div>
    </div>
  );
}

function ToolCallCard({ message }: { message: ChatMessage }) {
  const tc = message.toolCall!;
  return (
    <div className="border border-neutral-700 rounded-lg overflow-hidden">
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
  );
}
