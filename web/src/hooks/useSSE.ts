import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ChatMessage,
  ToolStartPayload,
  ToolEndPayload,
  ProviderInfo,
} from "../types/events";

interface ServerContentBlock {
  type: string;
  text?: string;
  thinking?: string;
  name?: string;
  arguments?: Record<string, any>;
  is_error?: boolean;
  tool_name?: string;
  tool_call_id?: string;
}

interface ServerMessage {
  role: string;
  content?: ServerContentBlock[];
  timestamp?: number;
  tool_call_id?: string;
  tool_name?: string;
  is_error?: boolean;
}

function transformServerMessages(msgs: ServerMessage[]): ChatMessage[] {
  const result: ChatMessage[] = [];
  for (const msg of msgs) {
    const ts = msg.timestamp || Date.now();
    switch (msg.role) {
      case "user": {
        const text = (msg.content || [])
          .filter((b) => b.type === "text")
          .map((b) => b.text || "")
          .join("");
        result.push({ id: crypto.randomUUID(), role: "user", content: text, timestamp: ts });
        break;
      }
      case "assistant": {
        for (const block of msg.content || []) {
          if (block.type === "thinking" && block.thinking) {
            result.push({ id: crypto.randomUUID(), role: "thinking", content: block.thinking, timestamp: ts });
          }
        }
        const text = (msg.content || [])
          .filter((b) => b.type === "text")
          .map((b) => b.text || "")
          .join("");
        if (text) {
          result.push({ id: crypto.randomUUID(), role: "assistant", content: text, timestamp: ts });
        }
        for (const block of msg.content || []) {
          if (block.type === "toolCall" && block.name) {
            result.push({
              id: crypto.randomUUID(),
              role: "tool",
              content: "",
              toolCall: { name: block.name, args: block.arguments || {}, isStreaming: false },
              timestamp: ts,
            });
          }
        }
        break;
      }
      case "toolResult": {
        const text = (msg.content || [])
          .filter((b) => b.type === "text")
          .map((b) => b.text || "")
          .join("");
        result.push({
          id: crypto.randomUUID(),
          role: "tool",
          content: text,
          toolCall: { name: msg.tool_name || "", args: {}, output: text, isError: msg.is_error || false, isStreaming: false },
          timestamp: ts,
        });
        break;
      }
    }
  }
  return result;
}

export function useSSE(baseUrl: string) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [connected, setConnected] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);

  useEffect(() => {
    setConnected(true);
    return () => {
      abortRef.current?.abort();
      eventSourceRef.current?.close();
    };
  }, []);

  const switchSession = useCallback(
    async (id: string) => {
      eventSourceRef.current?.close();
      abortRef.current?.abort();
      setSessionId(id);
      setMessages([]);
      setStreaming(false);
      setConnected(true);
      try {
        const res = await fetch(`${baseUrl}/sessions/${id}/messages?limit=1000`);
        if (res.ok) {
          const data = await res.json();
          setMessages(transformServerMessages(data.messages || []));
        }
      } catch (e) {
        console.error("Failed to load session messages:", e);
      }
    },
    [baseUrl]
  );

  const createSession = useCallback(
    async (modelId?: string, provider?: string) => {
      try {
        eventSourceRef.current?.close();
        abortRef.current?.abort();
        const res = await fetch(`${baseUrl}/sessions`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ model_id: modelId, provider }),
        });
        if (!res.ok) {
          console.error("Failed to create session:", res.status);
          return null;
        }
        const data = await res.json();
        setSessionId(data.session_id);
        setMessages([]);
        return data.session_id as string;
      } catch (e) {
        console.error("Failed to create session:", e);
        return null;
      }
    },
    [baseUrl]
  );

  const ask = useCallback(
    (prompt: string, thinkingLevel?: string) => {
      if (!sessionId) {
        console.warn("Cannot send: no active session. Create a new session first.");
        return;
      }

      setStreaming(true);
      setMessages((prev) => [
        ...prev,
        {
          id: crypto.randomUUID(),
          role: "user",
          content: prompt,
          timestamp: Date.now(),
        },
      ]);

      // Close previous SSE connection to prevent duplicate event sources
      eventSourceRef.current?.close();
      abortRef.current?.abort();

      const controller = new AbortController();
      abortRef.current = controller;

      const eventSource = new EventSource(`${baseUrl}/events/${sessionId}`);
      eventSourceRef.current = eventSource;

      let asked = false;
      let turnEndMessageCount = -1;

      eventSource.addEventListener("open", () => {
        setConnected(true);
        if (asked) return;
        asked = true;
        fetch(`${baseUrl}/ask`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          signal: controller.signal,
          body: JSON.stringify({
            session_id: sessionId,
            prompt,
            thinking_level: thinkingLevel,
          }),
        }).catch(() => {});
      });

      eventSource.addEventListener("message_update", (e) => {
        try {
          const payload = JSON.parse(e.data);
          setMessages((prev) => {
            let updated = [...prev];

            // -- Process thinking content first (so it's always before text in the array) --
            if (payload.thinking) {
              const last = updated[updated.length - 1];
              if (turnEndMessageCount >= 0) {
                let found = false;
                for (let i = updated.length - 1; i > turnEndMessageCount; i--) {
                  if (updated[i].role === "thinking") {
                    updated[i] = { ...updated[i], content: payload.thinking };
                    found = true;
                    break;
                  }
                }
                if (!found) {
                  updated = [...updated, {
                    id: crypto.randomUUID(),
                    role: "thinking",
                    content: payload.thinking,
                    timestamp: Date.now(),
                  }];
                }
              } else if (last?.role === "thinking") {
                updated[updated.length - 1] = { ...last, content: payload.thinking };
              } else if (last?.role === "assistant") {
                for (let i = updated.length - 1; i >= 0; i--) {
                  if (updated[i].role === "thinking") {
                    updated[i] = { ...updated[i], content: payload.thinking };
                    break;
                  }
                }
              } else {
                updated = [...updated, {
                  id: crypto.randomUUID(),
                  role: "thinking",
                  content: payload.thinking,
                  timestamp: Date.now(),
                }];
              }
            }

            // -- Process text content after thinking --
            if (payload.text) {
              const last = updated[updated.length - 1];
              if (turnEndMessageCount >= 0) {
                let found = false;
                for (let i = updated.length - 1; i > turnEndMessageCount; i--) {
                  if (updated[i].role === "assistant") {
                    updated[i] = { ...updated[i], content: payload.text };
                    found = true;
                    break;
                  }
                }
                if (!found) {
                  updated = [...updated, {
                    id: crypto.randomUUID(),
                    role: "assistant",
                    content: payload.text,
                    timestamp: Date.now(),
                  }];
                }
              } else if (last?.role === "assistant") {
                updated[updated.length - 1] = { ...last, content: payload.text };
              } else {
                updated = [...updated, {
                  id: crypto.randomUUID(),
                  role: "assistant",
                  content: payload.text,
                  timestamp: Date.now(),
                }];
              }
            }

            return updated;
          });
        } catch {}
      });

      eventSource.addEventListener("thinking", (e) => {
        try {
          const payload = JSON.parse(e.data);
          setMessages((prev) => {
            const last = prev[prev.length - 1];
            if (last?.role === "thinking") {
              const updated = [...prev];
              updated[updated.length - 1] = {
                ...last,
                content: payload.text,
              };
              return updated;
            }
            if (last?.role === "assistant") {
              if (turnEndMessageCount >= 0) {
                for (let i = prev.length - 1; i > turnEndMessageCount; i--) {
                  if (prev[i].role === "thinking") {
                    const updated = [...prev];
                    updated[i] = { ...updated[i], content: payload.text };
                    return updated;
                  }
                }
              } else {
                let lastUserIdx = -1;
                for (let i = prev.length - 1; i >= 0; i--) {
                  if (prev[i].role === "user") { lastUserIdx = i; break; }
                }
                for (let i = prev.length - 1; i > lastUserIdx; i--) {
                  if (prev[i].role === "thinking") {
                    const updated = [...prev];
                    updated[i] = {
                      ...updated[i],
                      content: payload.text,
                    };
                    return updated;
                  }
                }
              }
            }
            return [
              ...prev,
              {
                id: crypto.randomUUID(),
                role: "thinking",
                content: payload.text,
                timestamp: Date.now(),
              },
            ];
          });
        } catch {}
      });

      eventSource.addEventListener("tool_start", (e) => {
        try {
          const payload: ToolStartPayload = JSON.parse(e.data);
          setMessages((prev) => [
            ...prev,
            {
              id: payload.tool_call_id,
              role: "tool",
              content: "",
              toolCall: {
                name: payload.tool_name,
                args: payload.args,
                isStreaming: true,
              },
              timestamp: Date.now(),
            },
          ]);
        } catch {}
      });

      eventSource.addEventListener("tool_update", (e) => {
        try {
          const { tool_call_id, partial_output } = JSON.parse(e.data);
          setMessages((prev) =>
            prev.map((m) =>
              m.id === tool_call_id && m.toolCall
                ? { ...m, content: m.content + partial_output }
                : m
            )
          );
        } catch {}
      });

      eventSource.addEventListener("tool_end", (e) => {
        try {
          const payload: ToolEndPayload = JSON.parse(e.data);
          setMessages((prev) =>
            prev.map((m) =>
              m.id === payload.tool_call_id && m.toolCall
                ? {
                    ...m,
                    content: payload.output,
                    toolCall: {
                      ...m.toolCall,
                      output: payload.output,
                      isError: payload.is_error,
                      isStreaming: false,
                    },
                  }
                : m
            )
          );
        } catch {}
      });

      eventSource.addEventListener("turn_end", () => {
        setMessages((prev) => {
          turnEndMessageCount = prev.length;
          return prev;
        });
      });

      eventSource.addEventListener("agent_end", () => {
        turnEndMessageCount = -1;
        setStreaming(false);
        eventSource.close();
      });

      eventSource.addEventListener("error", () => {
        setConnected(false);
        setStreaming(false);
        eventSource.close();
      });

      controller.signal.addEventListener("abort", () => {
        eventSource.close();
      });
    },
    [baseUrl, sessionId]
  );

  const cancel = useCallback(() => {
    if (!sessionId) return;
    abortRef.current?.abort();
    eventSourceRef.current?.close();
    fetch(`${baseUrl}/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sessionId }),
    }).catch(() => {});
    setStreaming(false);
  }, [baseUrl, sessionId]);

  return {
    sessionId,
    messages,
    streaming,
    connected,
    createSession,
    switchSession,
    ask,
    cancel,
  };
}

export async function fetchProviders(baseUrl: string): Promise<ProviderInfo[]> {
  try {
    const res = await fetch(`${baseUrl}/providers`);
    return await res.json();
  } catch {
    return [];
  }
}
