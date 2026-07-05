import { useState, useEffect } from "react";
import { Plus, Trash2, RotateCw } from "lucide-react";

interface McpServer {
  name: string;
  transport: string;
  tool_count: number;
  tool_names: string[];
  prompt_count: number;
  prompt_names: string[];
  resource_count: number;
  resource_names: string[];
  is_connected: boolean;
}

interface EnvVar {
  key: string;
  value: string;
}

interface McpSectionProps {
  serverUrl: string;
}

export function McpSection({ serverUrl }: McpSectionProps) {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddForm, setShowAddForm] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    command: "",
    args: "",
    url: "",
    tool_name_prefix: "",
    scope: "global",
  });
  const [envVars, setEnvVars] = useState<EnvVar[]>([]);

  const fetchServers = async () => {
    try {
      setLoading(true);
      const res = await fetch(`${serverUrl}/mcp`);
      if (res.ok) {
        const data: McpServer[] = await res.json();
        setServers(data);
      }
    } catch {
      /* ignore */
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchServers();
  }, [serverUrl]);

  const resetForm = () => {
    setForm({ name: "", command: "", args: "", url: "", tool_name_prefix: "", scope: "global" });
    setEnvVars([]);
  };

  const handleAdd = async () => {
    const env: Record<string, string> = {};
    envVars.forEach((ev) => {
      if (ev.key) env[ev.key] = ev.value;
    });

    const args = form.args
      ? form.args.split(",").map((a) => a.trim()).filter(Boolean)
      : undefined;

    const body: Record<string, unknown> = {
      name: form.name,
      command: form.command || undefined,
      args: args,
      url: form.url || undefined,
      env: Object.keys(env).length > 0 ? env : undefined,
      tool_name_prefix: form.tool_name_prefix || undefined,
      scope: form.scope,
    };

    const res = await fetch(`${serverUrl}/mcp`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });

    if (res.ok) {
      setShowAddForm(false);
      resetForm();
      fetchServers();
    }
  };

  const handleDelete = async (name: string) => {
    const res = await fetch(`${serverUrl}/mcp/${encodeURIComponent(name)}`, {
      method: "DELETE",
    });
    if (res.ok || res.status === 204 || res.status === 200) {
      setDeleteConfirm(null);
      fetchServers();
    }
  };

  const handleReconnect = async (name: string) => {
    await fetch(`${serverUrl}/mcp/${encodeURIComponent(name)}/reconnect`, {
      method: "POST",
    });
    fetchServers();
  };

  const inputCls =
    "w-full px-2.5 py-1.5 rounded text-xs bg-[var(--surface-button)] text-[var(--text-primary)] border border-[var(--border-base)] outline-none focus:border-blue-500/50 transition-colors";
  const labelCls = "text-xs font-medium text-[var(--text-secondary)]";

  return (
    <div className="space-y-6">
      <div>
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-semibold text-[var(--text-primary)]">MCP Servers</h3>
          <div className="flex items-center gap-2">
            <button
              onClick={fetchServers}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80 transition-colors"
            >
              <RotateCw className="w-3.5 h-3.5" />
              Refresh
            </button>
            <button
              onClick={() => setShowAddForm(true)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium bg-blue-500/20 text-blue-400 border border-blue-500/30 hover:opacity-80 transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              Add Server
            </button>
          </div>
        </div>

        {loading ? (
          <div className="text-xs text-[var(--text-muted)] py-8 text-center">Loading...</div>
        ) : servers.length === 0 ? (
          <div className="text-xs text-[var(--text-muted)] py-8 text-center">
            No MCP servers configured. Click "Add Server" to connect one.
          </div>
        ) : (
          <div className="space-y-2">
            {servers.map((srv) => (
              <div
                key={srv.name}
                className="settings-card flex items-center justify-between gap-3"
              >
                <div className="flex items-center gap-3 min-w-0">
                  <span
                    className={`w-2 h-2 rounded-full flex-shrink-0 ${
                      srv.is_connected ? "bg-green-400" : "bg-neutral-500"
                    }`}
                  />
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-[var(--text-primary)] truncate">
                      {srv.name}
                    </div>
                    <div className="text-[11px] text-[var(--text-muted)] mt-0.5">
                      {srv.transport}
                      {srv.tool_count > 0 && ` · ${srv.tool_count} tools`}
                      {srv.is_connected ? " · Connected" : " · Disconnected"}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-1.5 flex-shrink-0">
                  {!srv.is_connected && (
                    <button
                      onClick={() => handleReconnect(srv.name)}
                      className="flex items-center gap-1 px-2 py-1 rounded text-[11px] font-medium bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80 transition-colors"
                    >
                      <RotateCw className="w-3 h-3" />
                      Reconnect
                    </button>
                  )}
                  <button
                    onClick={() => setDeleteConfirm(srv.name)}
                    className="flex items-center gap-1 px-2 py-1 rounded text-[11px] font-medium text-red-400 bg-red-500/10 border border-red-500/20 hover:opacity-80 transition-colors"
                  >
                    <Trash2 className="w-3 h-3" />
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Delete Confirm Dialog */}
      {deleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[var(--surface-secondary)] border border-[var(--border-base)] rounded-lg p-5 max-w-sm w-full mx-3 shadow-xl">
            <p className="text-sm text-[var(--text-primary)] mb-4">
              Delete MCP server <strong>{deleteConfirm}</strong>? This will also remove it from the settings file.
            </p>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setDeleteConfirm(null)}
                className="px-3 py-1.5 rounded text-xs font-medium bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => handleDelete(deleteConfirm)}
                className="px-3 py-1.5 rounded text-xs font-medium text-red-400 bg-red-500/20 border border-red-500/30 hover:opacity-80 transition-colors"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add Server Modal */}
      {showAddForm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-[var(--surface-secondary)] border border-[var(--border-base)] rounded-lg p-5 max-w-lg w-full mx-3 shadow-xl max-h-[85vh] overflow-y-auto">
            <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-4">Add MCP Server</h3>

            <div className="space-y-3">
              {/* Name */}
              <div>
                <label className={labelCls}>Name</label>
                <input
                  className={inputCls}
                  placeholder="my-server"
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                />
              </div>

              {/* Command */}
              <div>
                <label className={labelCls}>
                  Command <span className="text-[var(--text-muted)]">(for stdio transport)</span>
                </label>
                <input
                  className={inputCls}
                  placeholder="npx"
                  value={form.command}
                  onChange={(e) => setForm({ ...form, command: e.target.value })}
                />
              </div>

              {/* Args */}
              <div>
                <label className={labelCls}>
                  Args <span className="text-[var(--text-muted)]">(comma-separated)</span>
                </label>
                <input
                  className={inputCls}
                  placeholder="-y, @modelcontextprotocol/server-filesystem, ."
                  value={form.args}
                  onChange={(e) => setForm({ ...form, args: e.target.value })}
                />
              </div>

              {/* URL */}
              <div>
                <label className={labelCls}>
                  URL <span className="text-[var(--text-muted)]">(for HTTP/SSE transport)</span>
                </label>
                <input
                  className={inputCls}
                  placeholder="https://api.example.com/mcp"
                  value={form.url}
                  onChange={(e) => setForm({ ...form, url: e.target.value })}
                />
              </div>

              {/* Tool Name Prefix */}
              <div>
                <label className={labelCls}>Tool Name Prefix</label>
                <input
                  className={inputCls}
                  placeholder="my_"
                  value={form.tool_name_prefix}
                  onChange={(e) => setForm({ ...form, tool_name_prefix: e.target.value })}
                />
              </div>

              {/* Env Vars */}
              <div>
                <label className={labelCls}>Environment Variables</label>
                <div className="space-y-1.5 mt-1">
                  {envVars.map((ev, i) => (
                    <div key={i} className="flex items-center gap-1.5">
                      <input
                        className={`${inputCls} w-[120px]`}
                        placeholder="KEY"
                        value={ev.key}
                        onChange={(e) => {
                          const next = [...envVars];
                          next[i] = { ...next[i], key: e.target.value };
                          setEnvVars(next);
                        }}
                      />
                      <input
                        className={`${inputCls} flex-1`}
                        placeholder="VALUE"
                        value={ev.value}
                        onChange={(e) => {
                          const next = [...envVars];
                          next[i] = { ...next[i], value: e.target.value };
                          setEnvVars(next);
                        }}
                      />
                      <button
                        onClick={() => setEnvVars(envVars.filter((_, j) => j !== i))}
                        className="p-1 rounded text-neutral-500 hover:text-red-400 transition-colors"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  ))}
                  <button
                    onClick={() => setEnvVars([...envVars, { key: "", value: "" }])}
                    className="flex items-center gap-1 text-xs text-blue-400 hover:opacity-80 transition-colors"
                  >
                    <Plus className="w-3 h-3" /> Add env var
                  </button>
                </div>
              </div>

              {/* Scope */}
              <div>
                <label className={labelCls}>Save to</label>
                <div className="flex gap-3 mt-1">
                  {(["global", "project"] as const).map((s) => (
                    <label
                      key={s}
                      className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium cursor-pointer transition-colors ${
                        form.scope === s
                          ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
                          : "bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80"
                      }`}
                    >
                      <input
                        type="radio"
                        name="scope"
                        value={s}
                        checked={form.scope === s}
                        onChange={() => setForm({ ...form, scope: s })}
                        className="hidden"
                      />
                      {s === "global" ? "Global (~/.pick/)" : "Project (.pick/)"}
                    </label>
                  ))}
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-2 mt-5">
              <button
                onClick={() => { setShowAddForm(false); resetForm(); }}
                className="px-3 py-1.5 rounded text-xs font-medium bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAdd}
                disabled={!form.name}
                className="px-3 py-1.5 rounded text-xs font-medium bg-blue-500/20 text-blue-400 border border-blue-500/30 hover:opacity-80 transition-colors disabled:opacity-40"
              >
                Save
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
