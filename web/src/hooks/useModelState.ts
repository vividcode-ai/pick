import { useState, useCallback, useEffect } from "react";
import { fetchProviders } from "./useSessionManager";
import type { ProviderInfo } from "../types/events";
import { getSessionEntry } from "../stores/sessions";

const THINKING_KEY = "pick_thinking_level";

function loadString(key: string, fallback = ""): string {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

function saveString(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* noop */
  }
}

export function useModelState(baseUrl: string | null) {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [selectedProvider, setSelectedProvider] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState(() => loadString(THINKING_KEY, "off"));
  const [loaded, setLoaded] = useState(false);
  const [inited, setInited] = useState(false);

  const refreshProviders = useCallback(() => {
    if (!baseUrl) return;
    fetchProviders(baseUrl).then((res) => {
      setProviders(res.providers);
      if (!inited && res.last_model && res.last_provider) {
        setSelectedModel(res.last_model);
        setSelectedProvider(res.last_provider);
        setInited(true);
      }
      setLoaded(true);
    });
  }, [baseUrl, inited]);

  useEffect(() => {
    refreshProviders();
  }, [refreshProviders]);

  useEffect(() => {
    if (!loaded || !inited) return;
  }, [providers, selectedModel, selectedProvider, loaded, inited]);

  const handleModelChange = useCallback(
    (modelId: string, provider: string, onCancel?: () => void) => {
      setSelectedModel(modelId);
      setSelectedProvider(provider);
      setInited(true);
      onCancel?.();
    },
    []
  );

  const handleThinkingLevelChange = useCallback(
    (level: string) => {
      setThinkingLevel(level);
      saveString(THINKING_KEY, level);
    },
    []
  );

  const syncFromSession = useCallback(
    (sessionId: string) => {
      const session = getSessionEntry(sessionId);
      if (session) {
        if (session.modelId && session.provider) {
          setSelectedModel(session.modelId);
          setSelectedProvider(session.provider);
        }
        if (session.thinkingLevel) {
          setThinkingLevel(session.thinkingLevel);
        }
      }
    },
    []
  );

  return {
    providers,
    selectedModel,
    selectedProvider,
    thinkingLevel,
    setSelectedModel,
    setSelectedProvider,
    setThinkingLevel,
    handleModelChange,
    handleThinkingLevelChange,
    syncFromSession,
    refreshProviders,
  };
}
