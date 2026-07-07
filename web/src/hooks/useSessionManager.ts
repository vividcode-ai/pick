import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  ApprovalRequiredPayload,
  ChatMessage,
  GitInfo,
  ProviderInfo,
  ProvidersResponse,
  QuestionPayload,
  TodoItem,
  ToolStartPayload,
  ToolEndPayload,
} from "../types/events";
import { renameSessionEntry } from "../stores/sessions";

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

const EVICTION_TIMEOUT = 30000;

interface SessionData {
  messages: ChatMessage[];
  streaming: boolean;
  connected: boolean;
  todos?: TodoItem[];
  gitInfo?: GitInfo | null;
  pendingMessages?: string[];
  pendingApproval?: ApprovalRequiredPayload | null;
  pendingQuestion?: QuestionPayload | null;
}

export function useSessionManager(baseUrl: string) {
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [sessionData, setSessionData] = useState<Record<string, SessionData>>({});

  const activeData = activeSessionId ? sessionData[activeSessionId] : undefined;
  const activeMessages = activeData?.messages ?? [];
  const activeStreaming = activeData?.streaming ?? false;
  const activeConnected = activeData ? activeData.connected : true;
  const activeTodos = activeData?.todos ?? [];
  const activeGitInfo = activeData?.gitInfo ?? null;
  const activePendingMessages = activeData?.pendingMessages ?? [];

  const streamingSessions = useMemo(() => {
    const result: Record<string, boolean> = {};
    for (const [id, data] of Object.entries(sessionData)) {
      if (data.streaming) result[id] = true;
    }
    return result;
  }, [sessionData]);

  const sessionResourcesRef = useRef<Record<string, { eventSource: EventSource | null; abortController: AbortController | null }>>({});
  const evictionTimersRef = useRef<Record<string, ReturnType<typeof setTimeout>>>({});
  const activeSessionIdRef = useRef<string | null>(null);
  const sessionDataRef = useRef<Record<string, SessionData>>({});
  const evictedSessionsRef = useRef<Set<string>>(new Set());

  useEffect(() => { activeSessionIdRef.current = activeSessionId; }, [activeSessionId]);
  useEffect(() => { sessionDataRef.current = sessionData; }, [sessionData]);

  const updateSession = useCallback((id: string, updater: (prev: SessionData) => SessionData) => {
    setSessionData(prev => {
      const prevState = prev[id] || { messages: [], streaming: false, connected: true, todos: [], gitInfo: null, pendingMessages: [] };
      return { ...prev, [id]: updater(prevState) };
    });
  }, []);

  const cancelEvictionTimer = useCallback((id: string) => {
    if (evictionTimersRef.current[id]) {
      clearTimeout(evictionTimersRef.current[id]);
      delete evictionTimersRef.current[id];
    }
  }, []);

  const startEvictionTimer = useCallback((id: string) => {
    cancelEvictionTimer(id);
    evictionTimersRef.current[id] = setTimeout(() => {
      const data = sessionDataRef.current[id];
      if (data && !data.streaming) {
        updateSession(id, (prev) => ({
          ...prev,
          messages: [],
        }));
        evictedSessionsRef.current.add(id);
      }
      delete evictionTimersRef.current[id];
    }, EVICTION_TIMEOUT);
  }, [cancelEvictionTimer, updateSession]);

  const switchActiveSession = useCallback(async (id: string) => {
    const prevId = activeSessionIdRef.current;

    if (prevId && prevId !== id) {
      const prevState = sessionDataRef.current[prevId];
      if (!prevState?.streaming) {
        startEvictionTimer(prevId);
      }
    }

    cancelEvictionTimer(id);
    setActiveSessionId(id);

    const needsMessages = evictedSessionsRef.current.has(id) || !sessionDataRef.current[id];
    if (needsMessages) {
      try {
        const res = await fetch(`${baseUrl}/sessions/${id}/messages?limit=1000`);
        if (res.ok) {
          const data = await res.json();
          updateSession(id, (prev) => ({
            ...prev,
            messages: transformServerMessages(data.messages || []),
          }));
          evictedSessionsRef.current.delete(id);
        }
      } catch (e) {
        console.error("Failed to load session messages:", e);
      }
    }
  }, [baseUrl, startEvictionTimer, cancelEvictionTimer, updateSession]);

  const createSession = useCallback(async (modelId?: string, provider?: string) => {
    try {
      const prevId = activeSessionIdRef.current;
      if (prevId) {
        const prevState = sessionDataRef.current[prevId];
        if (!prevState?.streaming) {
          startEvictionTimer(prevId);
        }
      }

      const res = await fetch(`${baseUrl}/sessions`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model_id: modelId, provider }),
      });
      if (!res.ok) return null;
      const data = await res.json();
      const newId = data.session_id as string;

      setSessionData(prev => ({
        ...prev,
        [newId]: { messages: [], streaming: false, connected: true, todos: [], gitInfo: null, pendingMessages: [] },
      }));
      sessionResourcesRef.current[newId] = { eventSource: null, abortController: null };
      evictedSessionsRef.current.delete(newId);
      cancelEvictionTimer(newId);
      setActiveSessionId(newId);

      return { id: newId, title: data.title as string };
    } catch (e) {
      console.error("Failed to create session:", e);
      return null;
    }
  }, [baseUrl, startEvictionTimer, cancelEvictionTimer]);

  const switchSession = useCallback(async (id: string) => {
    await switchActiveSession(id);
  }, [switchActiveSession]);

  /** Send a prompt via POST to the server (used when EventSource is already open). */
  const sendPrompt = useCallback((sessionId: string, prompt: string, thinkingLevel?: string) => {
    fetch(`${baseUrl}/ask`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        session_id: sessionId,
        prompt,
        thinking_level: thinkingLevel,
      }),
    }).catch(() => {});
  }, [baseUrl]);

  const ask = useCallback((prompt: string, thinkingLevel?: string) => {
    const sessionId = activeSessionIdRef.current;
    if (!sessionId) return;

    cancelEvictionTimer(sessionId);

    // Add user message to local display immediately
    updateSession(sessionId, (prev) => ({
      ...prev,
      messages: [
        ...prev.messages,
        { id: crypto.randomUUID(), role: "user", content: prompt, timestamp: Date.now() },
      ],
    }));

    const currentData = sessionDataRef.current[sessionId];

    if (currentData?.streaming) {
      // Already streaming — server will queue this message
      updateSession(sessionId, (prev) => ({
        ...prev,
        pendingMessages: [...(prev.pendingMessages ?? []), prompt],
      }));
      sendPrompt(sessionId, prompt, thinkingLevel);
      return;
    }

    // Not streaming — need an EventSource connection to receive events
    updateSession(sessionId, (prev) => ({ ...prev, streaming: true }));

    const existing = sessionResourcesRef.current[sessionId];
    if (existing?.eventSource && existing.eventSource.readyState === EventSource.OPEN) {
      // EventSource already open and ready — POST immediately
      sendPrompt(sessionId, prompt, thinkingLevel);
      return;
    }

    // Need to create a fresh EventSource connection
    existing?.eventSource?.close();
    existing?.abortController?.abort();

    const controller = new AbortController();
    const eventSource = new EventSource(`${baseUrl}/events/${sessionId}`);
    sessionResourcesRef.current[sessionId] = { eventSource, abortController: controller };

    let turnEndMessageCount = -1;
    let asked = false;

    eventSource.addEventListener("open", () => {
      updateSession(sessionId, (prev) => ({ ...prev, connected: true }));
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
        updateSession(sessionId, (prev) => {
          let updated = [...prev.messages];
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
          return { ...prev, messages: updated };
        });
      } catch {}
    });

    eventSource.addEventListener("thinking", (e) => {
      try {
        const payload = JSON.parse(e.data);
        updateSession(sessionId, (prev) => {
          const last = prev.messages[prev.messages.length - 1];
          if (last?.role === "thinking") {
            const updated = [...prev.messages];
            updated[updated.length - 1] = { ...last, content: payload.text };
            return { ...prev, messages: updated };
          }
          if (last?.role === "assistant") {
            if (turnEndMessageCount >= 0) {
              for (let i = prev.messages.length - 1; i > turnEndMessageCount; i--) {
                if (prev.messages[i].role === "thinking") {
                  const updated = [...prev.messages];
                  updated[i] = { ...updated[i], content: payload.text };
                  return { ...prev, messages: updated };
                }
              }
            } else {
              let lastUserIdx = -1;
              for (let i = prev.messages.length - 1; i >= 0; i--) {
                if (prev.messages[i].role === "user") { lastUserIdx = i; break; }
              }
              for (let i = prev.messages.length - 1; i > lastUserIdx; i--) {
                if (prev.messages[i].role === "thinking") {
                  const updated = [...prev.messages];
                  updated[i] = { ...updated[i], content: payload.text };
                  return { ...prev, messages: updated };
                }
              }
            }
          }
          return {
            ...prev,
            messages: [...prev.messages, { id: crypto.randomUUID(), role: "thinking", content: payload.text, timestamp: Date.now() }],
          };
        });
      } catch {}
    });

    eventSource.addEventListener("tool_start", (e) => {
      try {
        const payload: ToolStartPayload = JSON.parse(e.data);
        updateSession(sessionId, (prev) => ({
          ...prev,
          messages: [...prev.messages, {
            id: payload.tool_call_id,
            role: "tool",
            content: "",
            toolCall: { name: payload.tool_name, args: payload.args, isStreaming: true },
            timestamp: Date.now(),
          }],
        }));
      } catch {}
    });

    eventSource.addEventListener("tool_update", (e) => {
      try {
        const { tool_call_id, partial_output } = JSON.parse(e.data);
        updateSession(sessionId, (prev) => ({
          ...prev,
          messages: prev.messages.map((m) =>
            m.id === tool_call_id && m.toolCall
              ? { ...m, content: m.content + partial_output }
              : m
          ),
        }));
      } catch {}
    });

    eventSource.addEventListener("tool_end", (e) => {
      try {
        const payload: ToolEndPayload = JSON.parse(e.data);
        updateSession(sessionId, (prev) => ({
          ...prev,
          messages: prev.messages.map((m) =>
            m.id === payload.tool_call_id && m.toolCall
              ? { ...m, content: payload.output, toolCall: { ...m.toolCall, output: payload.output, isError: payload.is_error, isStreaming: false } }
              : m
          ),
        }));
      } catch {}
    });

    eventSource.addEventListener("turn_end", () => {
      updateSession(sessionId, (prev) => {
        turnEndMessageCount = prev.messages.length;
        return prev;
      });
    });

    eventSource.addEventListener("todo_updated", (e) => {
      try {
        const data = JSON.parse(e.data);
        const todos: TodoItem[] = Array.isArray(data) ? data : data.todos || [];
        updateSession(sessionId, (prev) => ({ ...prev, todos }));
      } catch {}
    });

    eventSource.addEventListener("git_info_updated", (e) => {
      try {
        const gitInfo: GitInfo = JSON.parse(e.data);
        updateSession(sessionId, (prev) => ({ ...prev, gitInfo }));
      } catch {}
    });

    eventSource.addEventListener("message_dequeued", (e) => {
      try {
        const { text } = JSON.parse(e.data);
        if (!text) return;
        updateSession(sessionId, (prev) => {
          const msgs = prev.pendingMessages ?? [];
          const idx = msgs.indexOf(text);
          if (idx === -1) return prev;
          const next = [...msgs];
          next.splice(idx, 1);
          return { ...prev, pendingMessages: next };
        });
      } catch {}
    });

    eventSource.addEventListener("approval_required", (e) => {
      try {
        const payload: ApprovalRequiredPayload = JSON.parse(e.data);
        updateSession(sessionId, (prev) => ({ ...prev, pendingApproval: payload }));
      } catch {}
    });

    eventSource.addEventListener("question", (e) => {
      try {
        const payload: QuestionPayload = JSON.parse(e.data);
        updateSession(sessionId, (prev) => ({ ...prev, pendingQuestion: payload }));
      } catch {}
    });

    eventSource.addEventListener("agent_end", (e) => {
      turnEndMessageCount = -1;
      updateSession(sessionId, (prev) => ({ ...prev, streaming: false, pendingApproval: null, pendingQuestion: null }));
      if (sessionId !== activeSessionIdRef.current) {
        startEvictionTimer(sessionId);
      }
      // Keep EventSource open — reuse for subsequent asks

      try {
        const data = JSON.parse(e.data);
        if (data.title) {
          renameSessionEntry(sessionId, data.title);
        }
      } catch {}
    });

    eventSource.addEventListener("error", () => {
      updateSession(sessionId, (prev) => ({ ...prev, connected: false, streaming: false }));
      eventSource.close();
    });

    controller.signal.addEventListener("abort", () => {
      eventSource.close();
    });
  }, [baseUrl, cancelEvictionTimer, updateSession, startEvictionTimer, sendPrompt]);

  const cancel = useCallback(() => {
    const sessionId = activeSessionIdRef.current;
    if (!sessionId) return;

    // Keep EventSource alive — the server clears the queue and cancels the
    // current agent loop, but the SSE connection remains for subsequent asks.
    fetch(`${baseUrl}/cancel`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sessionId }),
    }).catch(() => {});

    updateSession(sessionId, (prev) => ({ ...prev, streaming: false, pendingMessages: [] }));
  }, [baseUrl, updateSession]);

  const respondApproval = useCallback(async (approvalId: string, approved: boolean) => {
    const sessionId = activeSessionIdRef.current;
    if (!sessionId) return;

    updateSession(sessionId, (prev) => ({ ...prev, pendingApproval: null }));

    fetch(`${baseUrl}/approve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sessionId, approval_id: approvalId, approved }),
    }).catch(() => {});
  }, [baseUrl, updateSession]);

  const answerQuestion = useCallback(async (questionId: string, answers: string[][]) => {
    const sessionId = activeSessionIdRef.current;
    if (!sessionId) return;

    updateSession(sessionId, (prev) => ({ ...prev, pendingQuestion: null }));

    fetch(`${baseUrl}/answer_question`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ session_id: sessionId, question_id: questionId, answers }),
    }).catch(() => {});
  }, [baseUrl, updateSession]);

  const deleteSession = useCallback(async (id: string) => {
    try {
      if (baseUrl) {
        await fetch(`${baseUrl}/sessions/${id}`, { method: "DELETE" });
      }
    } catch (e) {
      console.error("Failed to delete session on server:", e);
    }

    sessionResourcesRef.current[id]?.eventSource?.close();
    sessionResourcesRef.current[id]?.abortController?.abort();
    delete sessionResourcesRef.current[id];
    cancelEvictionTimer(id);
    evictedSessionsRef.current.delete(id);

    setSessionData(prev => {
      const next = { ...prev };
      delete next[id];
      return next;
    });

    if (activeSessionIdRef.current === id) {
      setActiveSessionId(null);
    }
  }, [baseUrl, cancelEvictionTimer]);

  const forkSession = useCallback(async (originSessionId: string, messageCount?: number) => {
    if (!baseUrl) return;
    try {
      const params = messageCount !== undefined ? `?message_count=${messageCount}` : '';
      const res = await fetch(`${baseUrl}/sessions/${originSessionId}/fork${params}`, { method: "POST" });
      if (!res.ok) return;
      const { session_id, title } = await res.json();

      setSessionData(prev => ({
        ...prev,
        [session_id]: { messages: [], streaming: false, connected: true, todos: [], gitInfo: null, pendingMessages: [] },
      }));
      sessionResourcesRef.current[session_id] = { eventSource: null, abortController: null };
      evictedSessionsRef.current.delete(session_id);

      await switchActiveSession(session_id);
      return { id: session_id, title };
    } catch (e) {
      console.error("Fork failed:", e);
    }
  }, [baseUrl, switchActiveSession]);

  useEffect(() => {
    return () => {
      for (const resources of Object.values(sessionResourcesRef.current)) {
        resources?.eventSource?.close();
        resources?.abortController?.abort();
      }
      for (const timer of Object.values(evictionTimersRef.current)) {
        clearTimeout(timer);
      }
    };
  }, []);

  const activePendingApproval = activeData?.pendingApproval ?? null;
  const activePendingQuestion = activeData?.pendingQuestion ?? null;

  return {
    activeSessionId,
    activeMessages,
    activeStreaming,
    activeConnected,
    activeTodos,
    activeGitInfo,
    activePendingMessages,
    activePendingApproval,
    activePendingQuestion,
    streamingSessions,
    createSession,
    switchSession,
    ask,
    cancel,
    respondApproval,
    answerQuestion,
    deleteSession,
    forkSession,
  };
}

export async function fetchProviders(baseUrl: string): Promise<ProvidersResponse> {
  try {
    const res = await fetch(`${baseUrl}/providers`);
    return await res.json();
  } catch {
    return { providers: [], last_provider: null, last_model: null, thinking_level: null };
  }
}
