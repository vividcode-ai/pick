import type { ProviderInfo } from "../../types/events";

interface ProvidersSectionProps {
  providers: ProviderInfo[];
  selectedModel: string;
  onModelChange: (modelId: string, provider: string) => void;
}

export function ProvidersSection({ providers, selectedModel, onModelChange }: ProvidersSectionProps) {
  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold text-neutral-100">AI Providers</h3>
        <p className="text-xs text-neutral-500 mt-1">Manage AI model providers and select models</p>
      </div>

      {providers.length === 0 ? (
        <p className="text-sm text-neutral-500">No providers available. Connect to a server first.</p>
      ) : (
        <div className="space-y-3">
          {providers.map((provider) => (
            <div key={provider.provider} className="settings-card">
              <div className="flex items-center justify-between">
                <span className="font-medium text-sm text-neutral-200">{provider.provider}</span>
                <span className={`text-xs ${provider.has_key ? "text-green-400" : "text-red-400"}`}>
                  {provider.has_key ? "Key configured" : "No API key"}
                </span>
              </div>
              <div className="mt-2 space-y-1">
                {provider.models.map((model) => (
                  <label
                    key={model.id}
                    className={`flex items-center gap-2 px-2 py-1.5 rounded text-sm cursor-pointer transition-colors ${
                      selectedModel === model.id
                        ? "bg-blue-500/10 text-blue-400"
                        : "text-neutral-400 hover:bg-neutral-800"
                    }`}
                  >
                    <input
                      type="radio"
                      name="model"
                      value={model.id}
                      checked={selectedModel === model.id}
                      onChange={() => onModelChange(model.id, provider.provider)}
                      className="accent-blue-500"
                    />
                    <span>{model.name}</span>
                    {model.reasoning && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-neutral-700 text-neutral-400">reasoning</span>
                    )}
                  </label>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
