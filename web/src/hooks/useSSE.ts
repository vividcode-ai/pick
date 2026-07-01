import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ChatMessage,
  ToolStartPayload,
  ToolEndPayload,
  ProviderInfo,
} from "../types/events";

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

  const createSession = useCallback(
    async (modelId?: string, provider?: string) => {
      try {
        const res = await fetch(`${baseUrl}/sessions`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ model_id: modelId, provider }),
        });
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
            const last = prev[prev.length - 1];
            if (last?.role === "assistant") {
              const updated = [...prev];
              updated[updated.length - 1] = {
                ...last,
                content: payload.text,
              };
              return updated;
            }
            if (last?.role === "thinking") {
              if (turnEndMessageCount >= 0) {
                for (let i = prev.length - 1; i > turnEndMessageCount; i--) {
                  if (prev[i].role === "assistant") {
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
                  if (prev[i].role === "assistant") {
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
                role: "assistant",
                content: payload.text,
                timestamp: Date.now(),
              },
            ];
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
