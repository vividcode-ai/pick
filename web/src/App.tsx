import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Layout } from "./components/Layout/Layout";
import { LeftPanel } from "./components/Layout/LeftPanel";
import { ChatView } from "./components/Chat/ChatView";
import { ChatInput } from "./components/Chat/ChatInput";
import { CommandPalette } from "./components/CommandPalette";
import { SettingsScreen } from "./components/Settings/SettingsScreen";
import { useTheme } from "./lib/ThemeProvider";
import { useSSE, fetchProviders } from "./hooks/useSSE";
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
} from "./stores/sessions";
import type { ProviderInfo } from "./types/events";

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
  const [settingsUrl, setSettingsUrl] = useState("");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState("off");
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
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
        }
      }
    });
  }, [baseUrl]);

  const {
    messages,
    streaming,
    connected,
    sessionId,
    createSession,
    ask,
    cancel,
  } = useSSE(baseUrl ?? "");

  const selectedProvider = useMemo(() => {
    for (const p of providers) {
      if (p.models.some((m) => m.id === selectedModel)) {
        return p.provider;
      }
    }
    return providers[0]?.provider || "anthropic";
  }, [providers, selectedModel]);

  const autoSessionRef = useRef(false);

  useEffect(() => {
    if (baseUrl && !sessionId && !autoSessionRef.current && providers.length > 0 && selectedModel) {
      autoSessionRef.current = true;
      createSession(selectedModel, selectedProvider).then((id) => {
        if (id) {
          addSessionEntry(id);
        } else {
          autoSessionRef.current = false;
        }
      });
    }
  }, [baseUrl, sessionId, providers, selectedModel, selectedProvider, createSession]);

  // Register commands
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

  // Track active session
  useEffect(() => {
    if (sessionId) {
      setActiveSessionId(sessionId);
    }
  }, [sessionId]);

  const handleSend = useCallback(
    (text: string) => {
      ask(text, thinkingLevel === "off" ? undefined : thinkingLevel);
    },
    [ask, thinkingLevel]
  );

  const handleNewSession = useCallback(() => {
    createSession(selectedModel, selectedProvider).then((id) => {
      if (id) {
        addSessionEntry(id);
      }
    });
  }, [createSession, selectedModel, selectedProvider]);

  const handleSelectSession = useCallback(
    (id: string) => {
      setActiveSessionId(id);
    },
    []
  );

  const handleRenameSession = useCallback(
    (id: string, title: string) => {
      renameSessionEntry(id, title);
    },
    []
  );

  const handleDeleteSession = useCallback(
    (id: string) => {
      removeSessionEntry(id);
      if (id === activeSessionId) {
        handleNewSession();
      }
    },
    [activeSessionId, handleNewSession]
  );

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

  const hasMessages = messages.length > 0;

  const chatInput = (
    <ChatInput
      onSend={handleSend}
      disabled={streaming}
      onCancel={cancel}
      connected={connected}
      providers={providers}
      selectedModel={selectedModel}
      onModelChange={setSelectedModel}
      thinkingLevel={thinkingLevel}
      onThinkingLevelChange={setThinkingLevel}
    />
  );

  return (
    <>
      <Layout
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((v) => !v)}
        rightPanelDiffs={[]}
        connected={connected}
        leftPanel={
          <LeftPanel
            onNewSession={handleNewSession}
            onSearch={() => {}}
            onPlugins={() => {}}
            onSettings={() => openSettings()}
            connected={connected}
            activeSessionId={activeSessionId}
            onSelectSession={handleSelectSession}
            onRenameSession={handleRenameSession}
            onDeleteSession={handleDeleteSession}
          />
        }
      >
        {hasMessages ? (
          <>
            <ChatView messages={messages} streaming={streaming} />
            {chatInput}
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center">
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
        onModelChange={setSelectedModel}
        serverUrl={settingsUrl}
        onSaveServerUrl={handleSaveUrl}
      />
    </>
  );
}
