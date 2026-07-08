import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { Layout } from "./components/Layout/Layout";
import { LeftPanel } from "./components/Layout/LeftPanel";
import { ChatView } from "./components/Chat/ChatView";
import { ChatInput } from "./components/Chat/ChatInput";
import { PickLogo } from "./components/PickLogo";
import { CommandPalette } from "./components/CommandPalette";
import { PermissionDialog } from "./components/Chat/PermissionDialog";
import { QuestionDialog } from "./components/Chat/QuestionDialog";
import { SettingsScreen } from "./components/Settings/SettingsScreen";
import { useTheme } from "./lib/ThemeProvider";
import { useSessionManager } from "./hooks/useSessionManager";
import { useModelState } from "./hooks/useModelState";
import { useCommandPalette } from "./hooks/useCommandPalette";
import {
  registerDefaultCommands,
  type Command,
} from "./stores/commands";
import {
  openSettings,
  subscribeToSettings,
  getSettingsSnapshot,
} from "./stores/settings";
import { initAppSettings } from "./stores/appSettings";
import {
  addSessionEntry,
  removeSessionEntry,
  renameSessionEntry,
  archiveSessionEntry,
  unarchiveSessionEntry,
  updateSessionEntry,
  initSessions,
} from "./stores/sessions";
import type { ChatMessage } from "./types/events";

async function detectBaseUrl(): Promise<string> {
  const params = new URLSearchParams(window.location.search);
  if (params.get("server")) return params.get("server")!;

  if (typeof window !== "undefined" && (window as any).__TAURI__) {
    try {
      const url = await (window as any).__TAURI__.invoke("get_server_url");
      if (url) return url.replace(/\/+$/, "");
    } catch {}
  }

  const origin = window.location.origin;
  const stored = localStorage.getItem("pick_server_url");
  return stored || origin;
}

export default function App() {
  const [baseUrl, setBaseUrl] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [sidebarPinned, setSidebarPinned] = useState(true);
  const [settingsUrl, setSettingsUrl] = useState("");
  const { cycleThemeMode } = useTheme();

  const {
    providers,
    selectedModel,
    selectedProvider,
    thinkingLevel,
    hiddenModels,
    setSelectedModel,
    setSelectedProvider,
    setThinkingLevel,
    syncFromSession,
    refreshProviders,
    toggleHiddenModel,
    ensureVisible,
  } = useModelState(baseUrl);

  useEffect(() => {
    detectBaseUrl().then((url) => {
      setBaseUrl(url);
      setSettingsUrl(url);
    });
  }, []);

  useEffect(() => {
    if (!baseUrl) return;
    initAppSettings(baseUrl);
  }, [baseUrl]);

  useEffect(() => {
    if (!baseUrl) return;
    fetch(`${baseUrl}/sessions`)
      .then((res) => (res.ok ? res.json() : []))
      .then((list: any[]) => {
        const entries = list.map((s) => ({
          id: s.id,
          title: s.title,
          createdAt: s.created_at,
          updatedAt: s.updated_at,
          modelId: s.model_id,
          provider: s.provider,
          thinkingLevel: s.thinking_level,
          archived: s.archived || false,
        }));
        if (entries.length > 0) initSessions(entries);
      })
      .catch(() => {});
  }, [baseUrl]);

  const {
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
  } = useSessionManager(baseUrl ?? "");

  const pendingSendRef = useRef<string | null>(null);

  useEffect(() => {
    if (activeSessionId && pendingSendRef.current !== null) {
      const text = pendingSendRef.current;
      pendingSendRef.current = null;
      ask(text, thinkingLevel === "off" ? undefined : thinkingLevel);
    }
  }, [activeSessionId, ask, thinkingLevel]);

  useEffect(() => {
    registerDefaultCommands({
      newSession: () => {
        createSession(selectedModel, selectedProvider).then((result) => {
          if (result) {
            addSessionEntry(result.id, result.title, selectedModel, selectedProvider, thinkingLevel);
          }
        });
      },
      toggleSidebar: () => setSidebarOpen((v) => !v),
      toggleTheme: cycleThemeMode,
      openSettings: () => openSettings(),
    });
  }, [createSession, selectedModel, selectedProvider, thinkingLevel, cycleThemeMode]);

  useEffect(() => {
    if (sidebarPinned) {
      setSidebarOpen(true);
    }
  }, [sidebarPinned]);

  const handleSend = useCallback(
    (text: string, opts?: { mode?: string; extraMode?: string | null }) => {
      if (!activeSessionId) {
        pendingSendRef.current = text;
        createSession(selectedModel, selectedProvider).then((result) => {
          if (result) {
            addSessionEntry(result.id, result.title, selectedModel, selectedProvider, thinkingLevel);
          } else {
            pendingSendRef.current = null;
          }
        });
      } else {
        ask(text, thinkingLevel === "off" ? undefined : thinkingLevel, opts?.extraMode ?? undefined);
      }
    },
    [activeSessionId, createSession, selectedModel, selectedProvider, ask, thinkingLevel]
  );

  const handleAsk = useCallback(
    (text: string) => {
      if (!activeSessionId) {
        pendingSendRef.current = text;
        createSession(selectedModel, selectedProvider).then((result) => {
          if (!result) pendingSendRef.current = null;
        });
      } else {
        ask(text);
      }
    },
    [activeSessionId, createSession, selectedModel, selectedProvider, ask]
  );

  const handleCommitRequest = useCallback(
    (message: string) => {
      if (activeSessionId) {
        ask(`Please commit the current code changes with message: ${message}. Only perform the commit, do nothing else.`, undefined);
      }
    },
    [activeSessionId, ask]
  );

  const handleModelChange = useCallback((modelId: string, provider: string) => {
    setSelectedModel(modelId);
    setSelectedProvider(provider);
    if (activeSessionId) {
      updateSessionEntry(activeSessionId, { modelId, provider });
    }
    cancel();
    if (baseUrl) {
      fetch(`${baseUrl}/last-model`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ provider, model: modelId }),
      }).catch(() => {});
    }
    if (activeSessionId && baseUrl) {
      fetch(`${baseUrl}/sessions/${activeSessionId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model_id: modelId, provider }),
      }).catch(() => {});
    }
  }, [activeSessionId, baseUrl, cancel]);

  const handleThinkingLevelChange = useCallback((level: string) => {
    setThinkingLevel(level);
    if (baseUrl) {
      fetch(`${baseUrl}/thinking-level`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ thinking_level: level }),
      }).catch(() => {});
    }
    if (activeSessionId) {
      updateSessionEntry(activeSessionId, { thinkingLevel: level });
    }
    if (activeSessionId && baseUrl) {
      fetch(`${baseUrl}/sessions/${activeSessionId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ thinking_level: level }),
      }).catch(() => {});
    }
  }, [activeSessionId, baseUrl]);

  const handleNewSession = useCallback(() => {
    createSession(selectedModel, selectedProvider).then((result) => {
      if (result) {
        addSessionEntry(result.id, result.title, selectedModel, selectedProvider, thinkingLevel);
      }
    });
  }, [createSession, selectedModel, selectedProvider, thinkingLevel]);

  const handleSelectSession = useCallback(
    (id: string) => {
      switchSession(id);
      syncFromSession(id);
    },
    [switchSession, syncFromSession]
  );

  const handleRenameSession = useCallback(
    (id: string, title: string) => {
      renameSessionEntry(id, title);
    },
    []
  );

  const handleDeleteSession = useCallback(
    async (id: string) => {
      await deleteSession(id);
      removeSessionEntry(id);
    },
    [deleteSession]
  );

  const handleArchiveSession = useCallback(
    async (id: string) => {
      try {
        if (baseUrl) {
          await fetch(`${baseUrl}/sessions/${id}`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ archived: true }),
          });
        }
      } catch (e) {
        console.error("Failed to archive session:", e);
      }
      archiveSessionEntry(id);
    },
    [baseUrl]
  );

  const handleUnarchiveSession = useCallback(
    async (id: string) => {
      try {
        if (baseUrl) {
          await fetch(`${baseUrl}/sessions/${id}`, {
            method: "PATCH",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ archived: false }),
          });
        }
      } catch (e) {
        console.error("Failed to unarchive session:", e);
      }
      unarchiveSessionEntry(id);
    },
    [baseUrl]
  );

  const handleDeleteArchivedSession = useCallback(
    async (id: string) => {
      await deleteSession(id);
      removeSessionEntry(id);
    },
    [deleteSession]
  );

  const handleFork = useCallback(async (message: ChatMessage) => {
    if (!activeSessionId || !baseUrl) return;
    const msgIdx = activeMessages.indexOf(message);
    if (msgIdx === -1) return;
    let userCount = 0;
    for (let i = 0; i <= msgIdx; i++) {
      if (activeMessages[i].role === "user") userCount++;
    }
    const result = await forkSession(activeSessionId, userCount);
    if (result) {
      addSessionEntry(result.id as string, result.title as string, selectedModel, selectedProvider, thinkingLevel);
    }
  }, [baseUrl, activeSessionId, forkSession, activeMessages, selectedModel, selectedProvider, thinkingLevel]);

  const handleSaveUrl = useCallback(
    (url: string) => {
      localStorage.setItem("pick_server_url", url);
      window.location.reload();
    },
    []
  );

  const { open: commandPaletteOpen, close: closeCommandPalette, commands } = useCommandPalette();
  const settingsState = useSyncExternalStore(subscribeToSettings, getSettingsSnapshot, getSettingsSnapshot);

  const handleExecuteCommand = useCallback(
    (cmd: Command) => {
      cmd.action();
    },
    []
  );

  if (!baseUrl) {
    return (
      <div className="flex h-screen items-center justify-center bg-neutral-950 text-neutral-400">
        Connecting...
      </div>
    );
  }

  if (settingsState.open) {
    return (
      <SettingsScreen
        serverUrl={settingsUrl}
        onSaveServerUrl={handleSaveUrl}
        onUnarchiveSession={handleUnarchiveSession}
        onDeleteArchivedSession={handleDeleteArchivedSession}
      />
    );
  }

  const hasMessages = activeMessages.length > 0;

  const inputSlot = activePendingApproval ? (
    <PermissionDialog
      payload={activePendingApproval}
      onRespond={(approved) => respondApproval(activePendingApproval.approval_id, approved)}
    />
  ) : activePendingQuestion ? (
    <QuestionDialog
      payload={activePendingQuestion}
      onSubmit={(answers) => answerQuestion(activePendingQuestion.question_id, answers)}
      onCancel={() => answerQuestion(activePendingQuestion.question_id, [])}
    />
  ) : (
    <ChatInput
      onSend={handleSend}
      disabled={activeStreaming}
      onCancel={cancel}
      connected={activeConnected}
      streaming={activeStreaming}
      providers={providers}
      selectedModel={selectedModel}
      selectedProvider={selectedProvider}
      onModelChange={handleModelChange}
      thinkingLevel={thinkingLevel}
      onThinkingLevelChange={handleThinkingLevelChange}
      sessionId={activeSessionId}
      pendingMessages={activePendingMessages}
      baseUrl={baseUrl ?? ""}
      onProvidersChange={refreshProviders}
      hiddenModels={hiddenModels}
      onToggleHidden={toggleHiddenModel}
      onEnsureVisible={ensureVisible}
    />
  );

  return (
    <>
      <Layout
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((v) => !v)}
        sidebarPinned={sidebarPinned}
        rightPanelDiffs={[]}
        connected={activeConnected}
        sessionId={activeSessionId}
        todos={activeTodos}
        gitInfo={activeGitInfo}
        onCommitRequest={handleCommitRequest}
        baseUrl={baseUrl}
        onAsk={handleAsk}
        provider={selectedProvider}
        modelId={selectedModel}
        leftPanel={
          <LeftPanel
            onNewSession={handleNewSession}
            onPlugins={() => {}}
            onSettings={() => openSettings()}
            connected={activeConnected}
            activeSessionId={activeSessionId}
            onSelectSession={handleSelectSession}
            onRenameSession={handleRenameSession}
            onArchiveSession={handleArchiveSession}
            streamingSessions={streamingSessions}
            pinned={sidebarPinned}
            onTogglePinned={() => setSidebarPinned((v) => !v)}
          />
        }
      >
        {hasMessages ? (
          <>
            <ChatView messages={activeMessages} onFork={handleFork} />
            {inputSlot}
          </>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center gap-6">
            <div className="w-full px-4">
              <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto flex justify-center">
                <PickLogo />
              </div>
            </div>
            {inputSlot}
          </div>
        )}
      </Layout>

      <CommandPalette
        open={commandPaletteOpen}
        onClose={closeCommandPalette}
        commands={commands}
        onExecute={handleExecuteCommand}
      />
    </>
  );
}
