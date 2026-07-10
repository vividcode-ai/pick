// Web-facing settings store synced with server via GET/PATCH /settings
// Follows the same pub/sub pattern as settings.ts

export interface WebSettings {
  // Display
  show_images: boolean;
  auto_resize_images: boolean;
  block_images: boolean;
  image_width_cells: number;

  // Behavior
  auto_compact: boolean;
  sandbox_enabled: boolean;
  mcp_tools: boolean;
  skill_commands: boolean;
  show_thinking: boolean;
  show_tool_calls: boolean;

  // Communication
  transport: string;
  http_idle_timeout_ms: number;

  // Agent
  steering_mode: string;
  follow_up_mode: string;

  // Startup & Privacy
  quiet_startup: boolean;
  collapse_changelog: boolean;
  install_telemetry: boolean;

  // Permission
  tool_execution_permission: "prompt" | "auto_approve";

  // Warnings
  anthropic_extra_usage: boolean;
}

const DEFAULTS: WebSettings = {
  show_images: true,
  auto_resize_images: true,
  block_images: false,
  image_width_cells: 60,

  auto_compact: true,
  sandbox_enabled: false,
  mcp_tools: true,
  skill_commands: true,
  show_thinking: true,
  show_tool_calls: true,

  transport: "auto",
  http_idle_timeout_ms: 300_000,

  steering_mode: "one-at-a-time",
  follow_up_mode: "one-at-a-time",

  quiet_startup: false,
  collapse_changelog: false,
  install_telemetry: false,

  tool_execution_permission: "prompt",

  anthropic_extra_usage: true,
};

let current: WebSettings = { ...DEFAULTS };
let baseUrl = "";
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

function saveToLocal() {
  try {
    localStorage.setItem("pick_web_settings", JSON.stringify(current));
  } catch {}
}

function loadFromLocal(): WebSettings | null {
  try {
    const raw = localStorage.getItem("pick_web_settings");
    if (raw) return JSON.parse(raw) as WebSettings;
  } catch {}
  return null;
}

// Map between Rust Settings JSON paths and WebSettings fields
interface RawSettings {
  terminal?: { show_images?: boolean; image_width_cells?: number };
  images?: { auto_resize?: boolean; block_images?: boolean };
  compaction?: { enabled?: boolean };
  permission?: { sandbox_enabled?: boolean };
  tool_execution_permission?: "prompt" | "auto_approve";
  enable_mcp_tools?: boolean;
  enable_skill_commands?: boolean;
  hide_thinking_block?: boolean;
  hide_tool_call_block?: boolean;
  quiet_startup?: boolean;
  collapse_changelog?: boolean;
  enable_install_telemetry?: boolean;
  transport?: string;
  http_idle_timeout_ms?: number;
  steering_mode?: string;
  follow_up_mode?: string;
  warnings?: { anthropic_extra_usage?: boolean };
}

function fromRust(raw: RawSettings): WebSettings {
  return {
    show_images: raw.terminal?.show_images ?? DEFAULTS.show_images,
    image_width_cells: raw.terminal?.image_width_cells ?? DEFAULTS.image_width_cells,
    auto_resize_images: raw.images?.auto_resize ?? DEFAULTS.auto_resize_images,
    block_images: raw.images?.block_images ?? DEFAULTS.block_images,
    auto_compact: raw.compaction?.enabled ?? DEFAULTS.auto_compact,
    sandbox_enabled: raw.permission?.sandbox_enabled ?? DEFAULTS.sandbox_enabled,
    tool_execution_permission: raw.tool_execution_permission ?? DEFAULTS.tool_execution_permission,
    mcp_tools: raw.enable_mcp_tools ?? DEFAULTS.mcp_tools,
    skill_commands: raw.enable_skill_commands ?? DEFAULTS.skill_commands,
    show_thinking: !(raw.hide_thinking_block ?? false),
    show_tool_calls: !(raw.hide_tool_call_block ?? false),
    transport: raw.transport ?? DEFAULTS.transport,
    http_idle_timeout_ms: raw.http_idle_timeout_ms ?? DEFAULTS.http_idle_timeout_ms,
    steering_mode: raw.steering_mode ?? DEFAULTS.steering_mode,
    follow_up_mode: raw.follow_up_mode ?? DEFAULTS.follow_up_mode,
    quiet_startup: raw.quiet_startup ?? DEFAULTS.quiet_startup,
    collapse_changelog: raw.collapse_changelog ?? DEFAULTS.collapse_changelog,
    install_telemetry: raw.enable_install_telemetry ?? DEFAULTS.install_telemetry,
    anthropic_extra_usage: raw.warnings?.anthropic_extra_usage ?? DEFAULTS.anthropic_extra_usage,
  };
}

function toRustPatch(ws: WebSettings): Record<string, unknown> {
  return {
    terminal: {
      show_images: ws.show_images,
      image_width_cells: ws.image_width_cells,
    },
    images: {
      auto_resize: ws.auto_resize_images,
      block_images: ws.block_images,
    },
    compaction: { enabled: ws.auto_compact },
    permission: { sandbox_enabled: ws.sandbox_enabled },
    tool_execution_permission: ws.tool_execution_permission,
    enable_mcp_tools: ws.mcp_tools,
    enable_skill_commands: ws.skill_commands,
    hide_thinking_block: !ws.show_thinking,
    hide_tool_call_block: !ws.show_tool_calls,
    quiet_startup: ws.quiet_startup,
    collapse_changelog: ws.collapse_changelog,
    enable_install_telemetry: ws.install_telemetry,
    transport: ws.transport,
    http_idle_timeout_ms: ws.http_idle_timeout_ms,
    steering_mode: ws.steering_mode,
    follow_up_mode: ws.follow_up_mode,
    warnings: { anthropic_extra_usage: ws.anthropic_extra_usage },
  };
}

export function initAppSettings(url: string) {
  baseUrl = url;

  // Try local cache, then fetch from server
  const cached = loadFromLocal();
  if (cached) {
    current = cached;
  }

  // Fetch from server to get latest
  fetch(`${baseUrl}/settings`)
    .then((r) => (r.ok ? r.json() : null))
    .then((data) => {
      if (data) {
        current = fromRust(data);
        saveToLocal();
        emit();
      }
    })
    .catch(() => {}); // Server may not have settings endpoint; keep locals
}

export function getAppSettings(): WebSettings {
  return current;
}

export function subscribeAppSettings(callback: () => void) {
  listeners.add(callback);
  return () => listeners.delete(callback);
}

export function setSetting<K extends keyof WebSettings>(key: K, value: WebSettings[K]) {
  current = { ...current, [key]: value };
  saveToLocal();
  emit();

  // Send PATCH to server (fire-and-forget)
  if (baseUrl) {
    fetch(`${baseUrl}/settings`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(toRustPatch(current)),
    }).catch(() => {});
  }
}
