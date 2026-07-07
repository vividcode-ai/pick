import { useState, useCallback, useEffect } from "react";
import { fetchProviders } from "./useSessionManager";
import type { ProviderInfo } from "../types/events";
import { updateSessionEntry, getSessionEntry } from "../stores/sessions";

const MODEL_KEY = "pick_selected_model";
const PROVIDER_KEY = "pick_selected_provider";
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
  const [selectedModel, setSelectedModel] = useState(() => loadString(MODEL_KEY));
  const [selectedProvider, setSelectedProvider] = useState(() => loadString(PROVIDER_KEY));
  const [thinkingLevel, setThinkingLevel] = useState(() => loadString(THINKING_KEY, "off"));
  const [loaded, setLoaded] = useState(false);

  const refreshProviders = useCallback(() => {
    if (!baseUrl) return;
    fetchProviders(baseUrl).then((list) => {
      setProviders(list);
      setLoaded(true);
    });
  }, [baseUrl]);

  useEffect(() => {
    refreshProviders();
  }, [refreshProviders]);

  useEffect(() => {
    if (!loaded || providers.length === 0) return;
    const currentProvider = providers.find((p) => p.provider === selectedProvider);
    if (currentProvider?.has_key) {
      const modelExists = currentProvider.models.some((m) => m.id === selectedModel);
      if (modelExists) return;
    }
    // 当前 provider 无 key 时保留 localStorage 选择，不重置
    if (!currentProvider?.has_key) return;
    const firstWithKey = providers.find((p) => p.has_key);
    if (firstWithKey && firstWithKey.models.length > 0) {
      setSelectedModel(firstWithKey.models[0].id);
      setSelectedProvider(firstWithKey.provider);
    } else {
      setSelectedModel("");
      setSelectedProvider("");
    }
  }, [providers, selectedModel, selectedProvider, loaded]);

  const handleModelChange = useCallback(
    (modelId: string, provider: string, onCancel?: () => void) => {
      setSelectedModel(modelId);
      setSelectedProvider(provider);
      saveString(MODEL_KEY, modelId);
      saveString(PROVIDER_KEY, provider);
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
