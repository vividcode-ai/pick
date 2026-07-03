import { useSyncExternalStore } from "react";
import { Laptop, Moon, Sun } from "lucide-react";
import { useTheme, type ThemeMode } from "../../lib/ThemeProvider";
import {
  initAppSettings,
  getAppSettings,
  subscribeAppSettings,
  setSetting,
  type WebSettings,
} from "../../stores/appSettings";
import { ToggleSetting } from "./ToggleSetting";
import { SelectSetting } from "./SelectSetting";

const themeOptions: { value: ThemeMode; icon: typeof Sun; label: string }[] = [
  { value: "system", icon: Laptop, label: "System" },
  { value: "light", icon: Sun, label: "Light" },
  { value: "dark", icon: Moon, label: "Dark" },
];

function useAppSettings() {
  return useSyncExternalStore(subscribeAppSettings, getAppSettings, getAppSettings);
}

const imageWidthOptions = [
  { value: "60", label: "60" },
  { value: "80", label: "80" },
  { value: "120", label: "120" },
];

const transportOptions = [
  { value: "auto", label: "Auto" },
  { value: "sse", label: "SSE" },
  { value: "websocket", label: "WS" },
  { value: "websocket-cached", label: "WS Cached" },
];

const httpTimeoutOptions = [
  { value: "30000", label: "30s" },
  { value: "60000", label: "1min" },
  { value: "300000", label: "5min" },
  { value: "600000", label: "10min" },
  { value: "1800000", label: "30min" },
  { value: "0", label: "Off" },
];

const steeringOptions = [
  { value: "one-at-a-time", label: "One at a time" },
  { value: "all", label: "All" },
];

const followUpOptions = [
  { value: "one-at-a-time", label: "One at a time" },
  { value: "all", label: "All" },
];

export function AppearanceSection() {
  const { themeMode, setThemeMode } = useTheme();
  const settings = useAppSettings();

  const toggle = (key: keyof WebSettings) => (val: boolean) => setSetting(key, val);
  const select = (key: keyof WebSettings) => (val: string) => {
    if (key === "http_idle_timeout_ms") {
      setSetting(key, Number(val));
    } else if (key === "image_width_cells") {
      setSetting(key, Number(val));
    } else {
      setSetting(key, val);
    }
  };

  return (
    <div className="space-y-6">
      {/* Display */}
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">Display</h3>
        <div className="settings-card space-y-0">
          <div className="settings-row">
            <div>
              <div className="settings-row-label">Theme</div>
              <div className="settings-row-description">Choose your preferred appearance</div>
            </div>
            <div className="flex gap-1.5 flex-shrink-0">
              {themeOptions.map((opt) => {
                const Icon = opt.icon;
                const selected = themeMode === opt.value;
                return (
                  <button
                    key={opt.value}
                    onClick={() => setThemeMode(opt.value)}
                    className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium transition-colors ${
                      selected
                        ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
                        : "bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80"
                    }`}
                  >
                    <Icon className="w-3.5 h-3.5" />
                    <span>{opt.label}</span>
                  </button>
                );
              })}
            </div>
          </div>
          <ToggleSetting
            label="Show images"
            description="Display inline images in chat"
            checked={settings.show_images}
            onChange={toggle("show_images")}
          />
          <ToggleSetting
            label="Auto-resize images"
            description="Automatically resize large images for display"
            checked={settings.auto_resize_images}
            onChange={toggle("auto_resize_images")}
          />
          <ToggleSetting
            label="Block images"
            description="Prevent images from being displayed"
            checked={settings.block_images}
            onChange={toggle("block_images")}
          />
          <SelectSetting
            label="Image width"
            description="Maximum width in cells for displayed images"
            options={imageWidthOptions}
            value={String(settings.image_width_cells)}
            onChange={select("image_width_cells")}
          />
        </div>
      </div>

      {/* Behavior */}
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">Behavior</h3>
        <div className="settings-card space-y-0">
          <ToggleSetting
            label="Auto-compact"
            description="Automatically compact context when approaching token limit"
            checked={settings.auto_compact}
            onChange={toggle("auto_compact")}
          />
          <ToggleSetting
            label="Sandbox"
            description="Run commands in a sandboxed environment"
            checked={settings.sandbox_enabled}
            onChange={toggle("sandbox_enabled")}
          />
          <ToggleSetting
            label="MCP tools"
            description="Enable Model Context Protocol tools"
            checked={settings.mcp_tools}
            onChange={toggle("mcp_tools")}
          />
          <ToggleSetting
            label="Skill commands"
            description="Enable /skill: command autocomplete"
            checked={settings.skill_commands}
            onChange={toggle("skill_commands")}
          />
          <ToggleSetting
            label="Show thinking"
            description="Display AI thinking/reasoning blocks"
            checked={settings.show_thinking}
            onChange={toggle("show_thinking")}
          />
        </div>
      </div>

      {/* Communication */}
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">Communication</h3>
        <div className="settings-card space-y-0">
          <SelectSetting
            label="Transport"
            description="Backend communication protocol"
            options={transportOptions}
            value={settings.transport}
            onChange={select("transport")}
          />
          <SelectSetting
            label="HTTP idle timeout"
            description="Time before idle HTTP connection is closed"
            options={httpTimeoutOptions}
            value={String(settings.http_idle_timeout_ms)}
            onChange={select("http_idle_timeout_ms")}
          />
        </div>
      </div>

      {/* Agent */}
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">Agent</h3>
        <div className="settings-card space-y-0">
          <SelectSetting
            label="Steering mode"
            description="How queued user messages are delivered to the agent"
            options={steeringOptions}
            value={settings.steering_mode}
            onChange={select("steering_mode")}
          />
          <SelectSetting
            label="Follow-up mode"
            description="How queued follow-up messages are delivered"
            options={followUpOptions}
            value={settings.follow_up_mode}
            onChange={select("follow_up_mode")}
          />
        </div>
      </div>

      {/* Startup & Privacy */}
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">Startup &amp; Privacy</h3>
        <div className="settings-card space-y-0">
          <ToggleSetting
            label="Quiet startup"
            description="Skip startup banner and tips"
            checked={settings.quiet_startup}
            onChange={toggle("quiet_startup")}
          />
          <ToggleSetting
            label="Collapse changelog"
            description="Collapse changelog section on startup"
            checked={settings.collapse_changelog}
            onChange={toggle("collapse_changelog")}
          />
          <ToggleSetting
            label="Install telemetry"
            description="Share anonymous install data to improve Pick"
            checked={settings.install_telemetry}
            onChange={toggle("install_telemetry")}
          />
        </div>
      </div>

      {/* Warnings */}
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-3">Warnings</h3>
        <div className="settings-card space-y-0">
          <ToggleSetting
            label="Anthropic extra usage"
            description="Show a warning when using Anthropic models with extra usage"
            checked={settings.anthropic_extra_usage}
            onChange={toggle("anthropic_extra_usage")}
          />
        </div>
      </div>
    </div>
  );
}
