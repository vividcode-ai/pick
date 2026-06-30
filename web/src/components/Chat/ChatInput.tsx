import { useState, useRef, useEffect, useMemo, type KeyboardEvent } from "react";
import type { ProviderInfo } from "../../types/events";

interface ChatInputProps {
  onSend: (text: string) => void;
  disabled: boolean;
  onCancel?: () => void;
  connected: boolean;
  providers: ProviderInfo[];
  selectedModel: string;
  onModelChange: (m: string) => void;
  thinkingLevel: string;
  onThinkingLevelChange: (l: string) => void;
}

const THINKING_LEVELS = [
  { value: "off", label: "Off" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
];

const PROVIDER_DISPLAY_NAMES: Record<string, string> = {
  "anthropic": "Anthropic",
  "amazon-bedrock": "Amazon Bedrock",
  "azure-openai-responses": "Azure OpenAI Responses",
  "cerebras": "Cerebras",
  "cloudflare-ai-gateway": "Cloudflare AI Gateway",
  "cloudflare-workers-ai": "Cloudflare Workers AI",
  "deepseek": "DeepSeek",
  "fireworks": "Fireworks",
  "google": "Google Gemini",
  "google-vertex": "Google Vertex AI",
  "groq": "Groq",
  "huggingface": "Hugging Face",
  "kimi-coding": "Kimi For Coding",
  "mistral": "Mistral",
  "minimax": "MiniMax",
  "minimax-cn": "MiniMax (China)",
  "moonshotai": "Moonshot AI",
  "moonshotai-cn": "Moonshot AI (China)",
  "opencode": "OpenCode Zen",
  "opencode-go": "OpenCode Go",
  "openai": "OpenAI",
  "openrouter": "OpenRouter",
  "together": "Together AI",
  "vercel-ai-gateway": "Vercel AI Gateway",
  "xai": "xAI",
  "zai": "Z.AI",
  "zai-coding-cn": "Z.AI Coding (China)",
  "nvidia": "NVIDIA",
  "openrouter-images": "OpenRouter Images",
  "xiaomi": "Xiaomi MiMo",
  "xiaomi-token-plan-cn": "Xiaomi MiMo Token Plan (China)",
  "xiaomi-token-plan-ams": "Xiaomi MiMo Token Plan (Amsterdam)",
  "xiaomi-token-plan-sgp": "Xiaomi MiMo Token Plan (Singapore)",
};

export function ChatInput({
  onSend,
  disabled,
  onCancel,
  connected,
  providers,
  selectedModel,
  onModelChange,
  thinkingLevel,
  onThinkingLevelChange,
}: ChatInputProps) {
  const [input, setInput] = useState("");
  const [currentCommand, setCurrentCommand] = useState<"build" | "plan">("build");
  const [commandOpen, setCommandOpen] = useState(false);
  const [modelSearchOpen, setModelSearchOpen] = useState(false);
  const [modelSearchQuery, setModelSearchQuery] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const commandRef = useRef<HTMLDivElement>(null);
  const modelSearchRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (commandRef.current && !commandRef.current.contains(e.target as Node)) {
        setCommandOpen(false);
      }
      if (modelSearchRef.current && !modelSearchRef.current.contains(e.target as Node)) {
        setModelSearchOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const allModels = useMemo(() => {
    return providers.flatMap((p) =>
      p.models.map((m) => ({
        ...m,
        provider: p.provider,
        hasKey: p.has_key,
      }))
    );
  }, [providers]);

  const selectedModelDetail = useMemo(() => {
    return allModels.find((m) => m.id === selectedModel) || null;
  }, [allModels, selectedModel]);

  const filteredModels = useMemo(() => {
    if (!modelSearchQuery) return allModels;
    const q = modelSearchQuery.toLowerCase();
    return allModels.filter(
      (m) =>
        m.name.toLowerCase().includes(q) ||
        m.id.toLowerCase().includes(q) ||
        m.provider.toLowerCase().includes(q)
    );
  }, [allModels, modelSearchQuery]);

  const groupedModels = useMemo(() => {
    const groups: Record<string, typeof filteredModels> = {};
    for (const m of filteredModels) {
      if (!groups[m.provider]) groups[m.provider] = [];
      groups[m.provider].push(m);
    }
    return groups;
  }, [filteredModels]);

  const handleSend = () => {
    const trimmed = input.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setInput("");
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    if (e.key === "Escape" && disabled && onCancel) {
      onCancel();
    }
  };

  const handleInput = () => {
    const el = textareaRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
    }
  };

  const insertCommand = (cmd: string) => {
    const newVal = cmd + " ";
    setInput(newVal);
    if (textareaRef.current) {
      textareaRef.current.focus();
      const len = newVal.length;
      textareaRef.current.setSelectionRange(len, len);
      handleInput();
    }
  };

  const executeCommand = (cmd: "build" | "plan") => {
    insertCommand(`/${cmd}`);
    setCurrentCommand(cmd);
    setCommandOpen(false);
  };

  const handleCommandKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Tab") {
      e.preventDefault();
      setCurrentCommand((prev) => (prev === "build" ? "plan" : "build"));
    }
    if (e.key === "Escape") {
      setCommandOpen(false);
    }
  };

  return (
    <div className="w-full px-4 py-3">
      <div className="max-w-[90%] md:max-w-[70%] lg:max-w-[40%] mx-auto rounded-2xl border border-neutral-700 bg-neutral-800">
        {/* Top: textarea */}
        <textarea
          ref={textareaRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onInput={handleInput}
          placeholder={connected ? "Type a message..." : "Connecting..."}
          rows={1}
          disabled={!connected}
          className="w-full bg-transparent text-neutral-100 px-4 pt-3 pb-3 text-sm resize-none outline-none placeholder-neutral-500 disabled:opacity-50 min-h-[44px]"
        />

        {/* Bottom: controls */}
        <div className="flex items-center justify-between px-3 pb-3 pt-1.5 border-t border-neutral-700/50">
          {/* Left: command dropdown */}
          <div className="relative" ref={commandRef}>
            <div className="flex">
              <button
                onClick={() => insertCommand(`/${currentCommand}`)}
                disabled={disabled || !connected}
                onKeyDown={handleCommandKeyDown}
                className="px-2.5 py-1 text-xs rounded-l-md bg-neutral-700 hover:bg-neutral-600 text-neutral-300 disabled:opacity-40 transition-colors"
              >
                {currentCommand.charAt(0).toUpperCase() + currentCommand.slice(1)}
              </button>
              <button
                onClick={() => setCommandOpen((v) => !v)}
                disabled={disabled || !connected}
                className="px-1 py-1 text-xs rounded-r-md bg-neutral-700 hover:bg-neutral-600 text-neutral-400 border-l border-neutral-600 disabled:opacity-40 transition-colors"
              >
                <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>
            </div>
            {commandOpen && (
              <div className="absolute bottom-full left-0 mb-1 w-24 rounded-md bg-neutral-800 border border-neutral-700 shadow-lg z-50 overflow-hidden">
                <button
                  onClick={() => executeCommand("build")}
                  className="w-full px-3 py-1.5 text-xs text-left text-neutral-300 hover:bg-neutral-700 transition-colors"
                >
                  Build
                </button>
                <button
                  onClick={() => executeCommand("plan")}
                  className="w-full px-3 py-1.5 text-xs text-left text-neutral-300 hover:bg-neutral-700 transition-colors"
                >
                  Plan
                </button>
              </div>
            )}
          </div>

          {/* Right: model selectors + send */}
          <div className="flex items-center gap-2">
            <div className="relative" ref={modelSearchRef}>
              <button
                onClick={() => { setModelSearchOpen(v => !v); setModelSearchQuery(""); }}
                disabled={disabled || allModels.length === 0}
                className="bg-neutral-700 text-neutral-200 text-xs rounded-md px-2 py-1 border border-neutral-600 outline-none max-w-[140px] disabled:opacity-40 flex items-center gap-1"
              >
                <span className="truncate flex-1">{selectedModelDetail?.name || "Model"}</span>
                <svg className="w-3 h-3 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                </svg>
              </button>
              {modelSearchOpen && (
                <div className="absolute bottom-full left-0 mb-1 w-64 max-h-64 rounded-md bg-neutral-800 border border-neutral-700 shadow-lg z-50 overflow-hidden flex flex-col">
                  <div className="p-1.5 border-b border-neutral-700">
                    <input
                      type="text"
                      value={modelSearchQuery}
                      onChange={(e) => setModelSearchQuery(e.target.value)}
                      placeholder="Search models..."
                      className="w-full bg-neutral-700 text-neutral-200 text-xs rounded px-2 py-1 outline-none placeholder-neutral-500"
                      autoFocus
                    />
                  </div>
                  <div className="overflow-y-auto">
                    {Object.entries(groupedModels).map(([provider, models]) => (
                      <div key={provider}>
                        <div className="px-3 py-1 text-xs text-neutral-400 font-medium sticky top-0 border-b border-neutral-700/50 bg-neutral-800">
                          {PROVIDER_DISPLAY_NAMES[provider] || provider}
                        </div>
                        {models.map((m) => (
                          <button
                            key={m.id}
                            onClick={() => { onModelChange(m.id); setModelSearchOpen(false); }}
                            className={`w-full px-3 py-1.5 text-xs text-left transition-colors ${
                              m.id === selectedModel
                                ? "bg-blue-600/30 text-blue-300"
                                : "text-neutral-300 hover:bg-neutral-700"
                            }`}
                          >
                            <span>{m.name}</span>
                            {!m.hasKey && <span className="text-neutral-500 ml-1">(no api key)</span>}
                          </button>
                        ))}
                      </div>
                    ))}
                    {Object.keys(groupedModels).length === 0 && (
                      <div className="px-3 py-2 text-xs text-neutral-500">No models found</div>
                    )}
                  </div>
                </div>
              )}
            </div>

            {selectedModelDetail?.reasoning && (
              <select
                value={thinkingLevel}
                onChange={(e) => onThinkingLevelChange(e.target.value)}
                disabled={disabled}
                className="bg-neutral-700 text-neutral-200 text-xs rounded-md px-2 py-1 border border-neutral-600 outline-none disabled:opacity-40 max-w-[80px]"
              >
                {THINKING_LEVELS.map((l) => (
                  <option key={l.value} value={l.value}>
                    {l.label}
                  </option>
                ))}
              </select>
            )}

            {disabled ? (
              <button
                onClick={onCancel}
                className="px-3 py-1.5 bg-red-600 text-white rounded-lg text-xs hover:bg-red-700 transition-colors"
              >
                Stop
              </button>
            ) : (
              <button
                onClick={handleSend}
                disabled={!input.trim() || !connected}
                className="px-3 py-1.5 bg-blue-600 text-white rounded-lg text-xs hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                Send
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
