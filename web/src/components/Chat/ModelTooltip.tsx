import type { ModelInfo } from "../../types/events";

interface ModelTooltipProps {
  model: ModelInfo;
  providerName: string;
}

export function ModelTooltip({ model, providerName }: ModelTooltipProps) {
  const inputTypes = model.capabilities?.input;
  const supports = inputTypes && inputTypes.length > 0 ? inputTypes.join(", ") : null;

  return (
    <div className="flex flex-col gap-1 py-0.5">
      <div className="text-xs font-medium text-[var(--text-primary)]">
        {providerName} — {model.name}
      </div>
      <div className="text-[11px] text-[var(--text-muted)]">
        {model.id}
      </div>
      {model.context && (
        <div className="text-[11px] text-[var(--text-muted)]">
          Context: {model.context.toLocaleString()} tokens
        </div>
      )}
      {supports && (
        <div className="text-[11px] text-[var(--text-muted)]">
          Supports: {supports}
        </div>
      )}
      <div className="text-[11px] text-[var(--text-muted)]">
        Reasoning: {model.reasoning ? "Available" : "Not available"}
      </div>
    </div>
  );
}
