import { useCallback, useEffect, useState } from "react";
import { Layout, SidebarContent, SidebarHeader } from "./components/Layout/Layout";
import { ChatView } from "./components/Chat/ChatView";
import { ChatInput } from "./components/Chat/ChatInput";
import { useAgentSession } from "./hooks/useWebSocket";

const DEFAULT_HTTP_URL = "http://localhost:8080";

async function detectServerUrl(): Promise<string> {
  const params = new URLSearchParams(window.location.search);
  if (params.get("server")) return params.get("server")!;

  if (typeof window !== "undefined" && (window as any).__TAURI__) {
    try {
      const url = await (window as any).__TAURI__.invoke("get_server_url");
      if (url) return url;
    } catch {}
  }

  // Derive from current page URL (covers pick server random port)
  const proto = window.location.protocol === "https:" ? "https:" : "http:";
  const derivedUrl = `${proto}//${window.location.host}`;

  return localStorage.getItem("pick_server_url") || derivedUrl;
}

export default function App() {
  const [httpUrl, setHttpUrl] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsUrl, setSettingsUrl] = useState("");

  useEffect(() => {
    detectServerUrl().then((url) => {
      setHttpUrl(url);
      setSettingsUrl(url);
    });
  }, []);

  const handleSaveUrl = useCallback(() => {
    localStorage.setItem("pick_server_url", settingsUrl);
    window.location.reload();
  }, [settingsUrl]);

  if (!httpUrl) {
    return (
      <div className="flex h-screen items-center justify-center bg-neutral-950 text-neutral-400">
        Connecting...
      </div>
    );
  }

  return (
    <>
      <AppContent httpUrl={httpUrl} onOpenSettings={() => setSettingsOpen(true)} />

      {settingsOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={() => setSettingsOpen(false)}>
          <div className="bg-neutral-900 border border-neutral-700 rounded-lg p-6 w-96" onClick={(e) => e.stopPropagation()}>
            <h2 className="text-lg font-semibold text-neutral-100 mb-4">Settings</h2>
            <label className="block text-sm text-neutral-400 mb-1">Server URL</label>
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
                Save &amp; Reload
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

interface AppContentProps {
  httpUrl: string;
  onOpenSettings: () => void;
}

function AppContent({ httpUrl, onOpenSettings }: AppContentProps) {
  const { messages, streaming, connected, createSession, ask, cancel } =
    useAgentSession(httpUrl);

  useEffect(() => {
    createSession();
  }, [createSession]);

  return (
    <Layout
      sidebar={
        <>
          <SidebarHeader>Pick</SidebarHeader>
          <SidebarContent>
            <div className="text-sm text-neutral-400 space-y-2">
              <div className="flex items-center gap-2">
                <span
                  className={`w-2 h-2 rounded-full ${
                    connected ? "bg-green-500" : "bg-red-500"
                  }`}
                />
                <span>{connected ? "Connected" : "Disconnected"}</span>
              </div>
              <p className="text-xs text-neutral-500 break-all">{httpUrl}</p>
              <button
                className="w-full mt-4 px-3 py-1.5 text-xs bg-neutral-800 border border-neutral-700 rounded hover:bg-neutral-700"
                onClick={onOpenSettings}
              >
                Settings
              </button>
            </div>
          </SidebarContent>
        </>
      }
    >
      <ChatView messages={messages} streaming={streaming} />
      <ChatInput
        onSend={ask}
        disabled={streaming}
        onCancel={cancel}
        connected={connected}
      />
    </Layout>
  );
}
