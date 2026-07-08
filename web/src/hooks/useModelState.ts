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

  // On mount, poll /health with exponential backoff until the server is ready,
  // then fetch providers. This handles the Tauri startup race condition where
  // the backend HTTP server is spawned as a background task and may not be
  // serving yet when React first mounts.
  // `loaded` is a dep so that once providers are fetched successfully, the
  // effect bails out even if `refreshProviders` reference changes (e.g. via
  // `inited` flipping) and stops polling — avoiding redundant fetches.
  useEffect(() => {
    if (!baseUrl || loaded) return;
    let cancelled = false;
    let attempts = 0;

    const poll = () => {
      if (cancelled) return;
      fetch(`${baseUrl}/health`)
        .then((res) => {
          if (cancelled) return;
          if (res.ok) {
            refreshProviders();
          } else if (attempts < 10) {
            scheduleNext();
          }
        })
        .catch(() => {
          if (!cancelled && attempts < 10) scheduleNext();
        });
    };

    const scheduleNext = () => {
      attempts++;
      const delay = Math.min(1000 * Math.pow(1.5, attempts - 1), 10000);
      setTimeout(poll, delay);
    };

    poll();
    return () => {
      cancelled = true;
    };
  }, [baseUrl, loaded, refreshProviders]);

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
