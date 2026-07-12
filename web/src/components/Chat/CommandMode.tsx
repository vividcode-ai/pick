import { useState } from "react";
import { Plus, Target, Repeat, Terminal, Clock, ListTodo, Code2 } from "lucide-react";

export type ExtraMode =
  | "goal"
  | "loop"
  | "loop-goal"
  | "loop-command"
  | "loop-shell"
  | "loop-ask"
  | null;

interface CommandModeProps {
  value: ExtraMode;
  onChange: (v: ExtraMode) => void;
  disabled: boolean;
  connected: boolean;
}

interface ModeOption {
  value: ExtraMode;
  label: string;
  icon: typeof Target;
  description: string;
}

const mainModes: ModeOption[] = [
  {
    value: "goal",
    label: "Goal",
    icon: Target,
    description: "Set a persistent goal for the agent to track",
  },
];

const loopModes: ModeOption[] = [
  {
    value: "loop",
    label: "Loop Prompt",
    icon: Repeat,
    description: "Recurring prompt with interval (default: immediate)",
  },
  {
    value: "loop-ask",
    label: "Loop Ask",
    icon: Clock,
    description: "Recurring prompt that waits for first interval",
  },
  {
    value: "loop-goal",
    label: "Loop Goal",
    icon: ListTodo,
    description: "Goal-driven loop with completion tools",
  },
  {
    value: "loop-command",
    label: "Loop Command",
    icon: Terminal,
    description: "Execute a command on interval",
  },
  {
    value: "loop-shell",
    label: "Loop Shell",
    icon: Code2,
    description: "Execute a shell command on interval",
  },
];

function findMode(value: ExtraMode): ModeOption | undefined {
  return [...mainModes, ...loopModes].find((m) => m.value === value);
}

const activeMode = findMode;

export function CommandMode({
  value,
  onChange,
  disabled,
  connected,
}: CommandModeProps) {
  const [open, setOpen] = useState(false);

  const active = value ? activeMode(value) : null;

  return (
    <div className="relative flex items-center">
      <button
        onClick={() => setOpen((v) => !v)}
        disabled={disabled || !connected}
        className="inline-flex items-center gap-1 cursor-pointer text-xs text-[var(--text-muted)] hover:bg-[var(--surface-hover)] rounded-md px-1.5 py-0.5"
      >
        {active ? (
          <>
            <active.icon className="w-3.5 h-3.5" />
            {active.label}
          </>
        ) : (
          <Plus className="w-4 h-4" />
        )}
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-[2199]" onClick={() => setOpen(false)} />
          <div className="absolute bottom-full left-0 mb-1 w-44 rounded-md bg-[var(--surface-elevated)] border border-[var(--border-base)] shadow-lg z-[2200] overflow-hidden py-1">
            {/* Main modes */}
            {mainModes.map((mode) => (
              <button
                key={mode.value}
                onClick={() => { onChange(mode.value); setOpen(false); }}
                className={`w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left transition-colors ${
                  value === mode.value
                    ? "bg-[var(--surface-hover)] text-[var(--text-primary)]"
                    : "text-[var(--text-primary)] hover:bg-[var(--surface-hover)]"
                }`}
              >
                <mode.icon className="w-3.5 h-3.5 shrink-0" />
                <span>{mode.label}</span>
              </button>
            ))}

            {/* Separator */}
            <div className="mx-2 my-1 border-t border-[var(--border-base)]" />

            {/* Loop modes group label */}
            <div className="px-3 py-1 text-[10px] text-[var(--text-muted)] font-medium uppercase tracking-wider">
              Loop
            </div>

            {loopModes.map((mode) => (
              <button
                key={mode.value}
                onClick={() => { onChange(mode.value); setOpen(false); }}
                className={`w-full flex items-center gap-2 px-3 py-1.5 text-xs text-left transition-colors ${
                  value === mode.value
                    ? "bg-[var(--surface-hover)] text-[var(--text-primary)]"
                    : "text-[var(--text-primary)] hover:bg-[var(--surface-hover)]"
                }`}
              >
                <mode.icon className="w-3.5 h-3.5 shrink-0" />
                <div className="flex flex-col gap-0">
                  <span>{mode.label}</span>
                  <span className="text-[10px] text-[var(--text-muted)] leading-tight">
                    {mode.description}
                  </span>
                </div>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
