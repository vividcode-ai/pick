import { useState, useCallback, useEffect } from "react";
import { fetchProviders } from "./useSessionManager";
import type { ProviderInfo } from "../types/events";
import { getSessionEntry } from "../stores/sessions";

export function useModelState(baseUrl: string | null) {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  const [selectedProvider, setSelectedProvider] = useState("");
  const [thinkingLevel, setThinkingLevel] = useState("off");
  const [hiddenModels, setHiddenModels] = useState<string[]>([]);
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
      if (res.thinking_level) {
        setThinkingLevel(res.thinking_level);
      }
      setLoaded(true);
    });
    fetch(`${baseUrl}/settings`)
      .then((r) => r.ok ? r.json() : null)
      .then((data) => {
        if (data && Array.isArray(data.hidden_models)) {
          setHiddenModels(data.hidden_models);
        }
      })
      .catch(() => {});
  }, [baseUrl, inited]);

  useEffect(() => {
    refreshProviders();
  }, [refreshProviders]);

  const toggleHiddenModel = useCallback((key: string) => {
    setHiddenModels((prev) => {
      const next = prev.includes(key)
        ? prev.filter((k) => k !== key)
        : [...prev, key];
      if (baseUrl) {
        fetch(`${baseUrl}/settings`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ hidden_models: next }),
        }).catch(() => {});
      }
      return next;
    });
  }, [baseUrl]);

  const ensureVisible = useCallback((key: string) => {
    setHiddenModels((prev) => {
      if (!prev.includes(key)) return prev;
      const next = prev.filter((k) => k !== key);
      if (baseUrl) {
        fetch(`${baseUrl}/settings`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ hidden_models: next }),
        }).catch(() => {});
      }
      return next;
    });
  }, [baseUrl]);

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
    hiddenModels,
    setSelectedModel,
    setSelectedProvider,
    setThinkingLevel,
    handleModelChange,
    handleThinkingLevelChange,
    syncFromSession,
    refreshProviders,
    toggleHiddenModel,
    ensureVisible,
  };
}
