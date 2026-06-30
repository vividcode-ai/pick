import { useCallback, useEffect, useMemo, useState } from "react";
import { Layout } from "./components/Layout/Layout";
import { LeftPanel } from "./components/Layout/LeftPanel";
import { ChatView } from "./components/Chat/ChatView";
import { ChatInput } from "./components/Chat/ChatInput";
import { useSSE, fetchProviders } from "./hooks/useSSE";
import type { ProviderInfo} from "./types/events";

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
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsUrl, setSettingsUrl] = useState("");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProvider, setSelectedProvider] = useState("anthropic");
  const [selectedModel, setSelectedModel] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState("off");

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
        setSelectedProvider(list[0].provider);
        if (list[0].models.length > 0) {
          setSelectedModel(list[0].models[0].id);
        }
      }
    });
  }, [baseUrl]);

  const selectedModelDetail = useMemo(() => {
    for (const p of providers) {
      if (p.provider === selectedProvider) {
        return p.models.find((m) => m.id === selectedModel) || null;
      }
    }
    return null;
  }, [providers, selectedProvider, selectedModel]);

  const modelsForProvider = useMemo(() => {
    return providers.find((p) => p.provider === selectedProvider)?.models || [];
  }, [providers, selectedProvider]);

  const handleSaveUrl = useCallback(() => {
    localStorage.setItem("pick_server_url", settingsUrl);
    window.location.reload();
  }, [settingsUrl]);

  if (!baseUrl) {
    return (
      <div className="flex h-screen items-center justify-center bg-neutral-950 text-neutral-400">
        Connecting...
      </div>
    );
  }

  return (
    <>
      <AppContent
        baseUrl={baseUrl}
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((v) => !v)}
        onOpenSettings={() => setSettingsOpen(true)}
        providers={providers}
        selectedProvider={selectedProvider}
        onProviderChange={setSelectedProvider}
        selectedModel={selectedModel}
        onModelChange={setSelectedModel}
        modelsForProvider={modelsForProvider}
        selectedModelDetail={selectedModelDetail}
        thinkingLevel={thinkingLevel}
        onThinkingLevelChange={setThinkingLevel}
      />

      {settingsOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
          onClick={() => setSettingsOpen(false)}
        >
          <div
            className="bg-neutral-900 border border-neutral-700 rounded-lg p-6 w-96"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-lg font-semibold text-neutral-100 mb-4">
              Settings
            </h2>
            <label className="block text-sm text-neutral-400 mb-1">
              Server URL
            </label>
            <input
              className="w-full px-3 py-2 bg-neutral-800 border border-neutral-700 rounded text-neutral-100 text-sm mb-4"
              value={settingsUrl}
              onChange={(e) => setSettingsUrl(e.target.value)}
            />
            <div className="flex justify-end gap-2">
              <button
                className="px-4 py-2 text-sm text-neutral-400 hover:text-neutral-200"
                onClick={() => setSettingsOpen(false)}
              >
                Cancel
              </button>
              <button
                className="px-4 py-2 text-sm bg-blue-600 text-white rounded hover:bg-blue-700"
                onClick={handleSaveUrl}
              >
                Save & Reload
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

interface AppContentProps {
  baseUrl: string;
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  onOpenSettings: () => void;
  providers: ProviderInfo[];
  selectedProvider: string;
  onProviderChange: (p: string) => void;
  selectedModel: string;
  onModelChange: (m: string) => void;
  modelsForProvider: { id: string; name: string; reasoning: boolean }[];
  selectedModelDetail: { id: string; name: string; reasoning: boolean } | null;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
}

function AppContent({
  baseUrl,
  sidebarOpen,
  onToggleSidebar,
  onOpenSettings,
  providers,
  selectedProvider,
  onProviderChange,
  selectedModel,
  onModelChange,
  modelsForProvider,
  selectedModelDetail,
  thinkingLevel,
  onThinkingLevelChange,
}: AppContentProps) {
  const {
    messages,
    streaming,
    connected,
    createSession,
    ask,
    cancel,
  } = useSSE(baseUrl);

  useEffect(() => {
    createSession(selectedModel, selectedProvider);
  }, [baseUrl]);

  const handleSend = useCallback(
    (text: string) => {
      ask(text, thinkingLevel === "off" ? undefined : thinkingLevel);
    },
    [ask, thinkingLevel]
  );

  const handleNewSession = useCallback(() => {
    createSession(selectedModel, selectedProvider);
  }, [createSession, selectedModel, selectedProvider]);

  const hasMessages = messages.length > 0;

  return (
    <Layout
      sidebarOpen={sidebarOpen}
      onToggleSidebar={onToggleSidebar}
      leftPanel={
        <LeftPanel
          onNewSession={handleNewSession}
          onSearch={() => {}}
          onPlugins={() => {}}
          onSettings={onOpenSettings}
          connected={connected}
        />
      }
    >
      {hasMessages ? (
        <>
          <ChatView messages={messages} streaming={streaming} />
          <ChatInput
            onSend={handleSend}
            disabled={streaming}
            onCancel={cancel}
            connected={connected}
            providers={providers}
            selectedProvider={selectedProvider}
            onProviderChange={onProviderChange}
            selectedModel={selectedModel}
            onModelChange={onModelChange}
            modelsForProvider={modelsForProvider}
            selectedModelDetail={selectedModelDetail}
            thinkingLevel={thinkingLevel}
            onThinkingLevelChange={onThinkingLevelChange}
          />
        </>
      ) : (
        <div className="flex-1 flex items-center justify-center px-4">
          <ChatInput
            onSend={handleSend}
            disabled={streaming}
            onCancel={cancel}
            connected={connected}
            providers={providers}
            selectedProvider={selectedProvider}
            onProviderChange={onProviderChange}
            selectedModel={selectedModel}
            onModelChange={onModelChange}
            modelsForProvider={modelsForProvider}
            selectedModelDetail={selectedModelDetail}
            thinkingLevel={thinkingLevel}
            onThinkingLevelChange={onThinkingLevelChange}
          />
        </div>
      )}
    </Layout>
  );
}
