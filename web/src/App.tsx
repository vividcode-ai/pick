import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Layout } from "./components/Layout/Layout";
import { LeftPanel } from "./components/Layout/LeftPanel";
import { ChatView } from "./components/Chat/ChatView";
import { ChatInput } from "./components/Chat/ChatInput";
import { PickLogo } from "./components/PickLogo";
import { CommandPalette } from "./components/CommandPalette";
import { SettingsScreen } from "./components/Settings/SettingsScreen";
import { useTheme } from "./lib/ThemeProvider";
import { useSessionManager, fetchProviders } from "./hooks/useSessionManager";
import { useCommandPalette } from "./hooks/useCommandPalette";
import {
  registerDefaultCommands,
  type Command,
} from "./stores/commands";
import {
  openSettings,
} from "./stores/settings";
import {
  addSessionEntry,
  removeSessionEntry,
  renameSessionEntry,
  initSessions,
} from "./stores/sessions";
import type { ChatMessage, ProviderInfo } from "./types/events";

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
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [selectedProvider, setSelectedProvider] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState("off");
  const { cycleThemeMode } = useTheme();

  useEffect(() => {
    detectBaseUrl().then((url) => {
      setBaseUrl(url);
      setSettingsUrl(url);
    });
  }, []);

  useEffect(() => {
    if (!baseUrl) return;
    fetchProviders(baseUrl).then((list) => {
      setProviders(list);
      if (list.length > 0) {
        const firstWithKey = list.find((p) => p.has_key) || list[0];
        if (firstWithKey.models.length > 0) {
          setSelectedModel(firstWithKey.models[0].id);
          setSelectedProvider(firstWithKey.provider);
        }
      }
    });
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
    streamingSessions,
    createSession,
    switchSession,
    ask,
    cancel,
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
        createSession(selectedModel, selectedProvider);
      },
      toggleSidebar: () => setSidebarOpen((v) => !v),
      toggleTheme: cycleThemeMode,
      openSettings: () => openSettings(),
    });
  }, [createSession, selectedModel, selectedProvider, cycleThemeMode]);

  const handleSend = useCallback(
    (text: string) => {
      if (!activeSessionId) {
        pendingSendRef.current = text;
        createSession(selectedModel, selectedProvider).then((result) => {
          if (result) {
            addSessionEntry(result.id, result.title);
          } else {
            pendingSendRef.current = null;
          }
        });
      } else {
        ask(text, thinkingLevel === "off" ? undefined : thinkingLevel);
      }
    },
    [activeSessionId, createSession, selectedModel, selectedProvider, ask, thinkingLevel]
  );

  const handleModelChange = useCallback((modelId: string, provider: string) => {
    setSelectedModel(modelId);
    setSelectedProvider(provider);
    cancel();
    if (activeSessionId && baseUrl) {
      fetch(`${baseUrl}/sessions/${activeSessionId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model_id: modelId, provider }),
      }).catch(() => {});
    }
  }, [activeSessionId, baseUrl, cancel]);

  const handleNewSession = useCallback(() => {
    createSession(selectedModel, selectedProvider).then((result) => {
      if (result) {
        addSessionEntry(result.id, result.title);
      }
    });
  }, [createSession, selectedModel, selectedProvider]);

  const handleSelectSession = useCallback(
    (id: string) => {
      switchSession(id);
    },
    [switchSession]
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
      addSessionEntry(result.id as string, result.title as string);
    }
  }, [baseUrl, activeSessionId, forkSession, activeMessages]);

  const handleSaveUrl = useCallback(
    (url: string) => {
      localStorage.setItem("pick_server_url", url);
      window.location.reload();
    },
    []
  );

  const { open: commandPaletteOpen, close: closeCommandPalette, commands } = useCommandPalette();

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

  const hasMessages = activeMessages.length > 0;

  const chatInput = (
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
      onThinkingLevelChange={setThinkingLevel}
      sessionId={activeSessionId}
    />
  );

  return (
    <>
      <Layout
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((v) => !v)}
        sidebarPinned={sidebarPinned}
        onToggleSidebarPinned={() => setSidebarPinned((v) => !v)}
        rightPanelDiffs={[]}
        connected={activeConnected}
        leftPanel={
          <LeftPanel
            onNewSession={handleNewSession}
            onPlugins={() => {}}
            onSettings={() => openSettings()}
            connected={activeConnected}
            activeSessionId={activeSessionId}
            onSelectSession={handleSelectSession}
            onRenameSession={handleRenameSession}
            onDeleteSession={handleDeleteSession}
            streamingSessions={streamingSessions}
            pinned={sidebarPinned}
            onTogglePinned={() => setSidebarPinned((v) => !v)}
          />
        }
      >
        {hasMessages ? (
          <>
            <ChatView messages={activeMessages} onFork={handleFork} />
            {chatInput}
          </>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center gap-6">
            <div className="w-full px-4">
              <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto flex justify-center">
                <PickLogo />
              </div>
            </div>
            {chatInput}
          </div>
        )}
      </Layout>

      <CommandPalette
        open={commandPaletteOpen}
        onClose={closeCommandPalette}
        commands={commands}
        onExecute={handleExecuteCommand}
      />

      <SettingsScreen
        providers={providers}
        selectedModel={selectedModel}
        onModelChange={handleModelChange}
        serverUrl={settingsUrl}
        onSaveServerUrl={handleSaveUrl}
      />
    </>
  );
}
