import { useState, useEffect } from "react";
import { Plus, Trash2, RotateCw, ChevronRight, Loader2 } from "lucide-react";

interface McpServer {
  name: string;
  transport: string;
  tool_count: number;
  tool_names: string[];
  tool_descriptions: string[];
  prompt_count: number;
  prompt_names: string[];
  prompt_descriptions: string[];
  resource_count: number;
  resource_names: string[];
  resource_descriptions: string[];
  is_connected: boolean;
}

interface EnvVar {
  key: string;
  value: string;
}

type TransportTab = "stdio" | "http";
type AuthType = "none" | "bearer" | "oauth2";

interface McpSectionProps {
  serverUrl: string;
}

export function McpSection({ serverUrl }: McpSectionProps) {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [showAddForm, setShowAddForm] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);

  // Common fields
  const [formName, setFormName] = useState("");
  const [formPrefix, setFormPrefix] = useState("");
  const [formScope, setFormScope] = useState("global");

  // Stdio fields
  const [formCommand, setFormCommand] = useState("");
  const [formArgs, setFormArgs] = useState("");
  const [envVars, setEnvVars] = useState<EnvVar[]>([]);

  // HTTP fields
  const [formUrl, setFormUrl] = useState("");
  const [authType, setAuthType] = useState<AuthType>("none");
  const [bearerToken, setBearerToken] = useState("");
  const [oauthClientId, setOauthClientId] = useState("");
  const [oauthClientSecret, setOauthClientSecret] = useState("");
  const [oauthScopes, setOauthScopes] = useState("");
  const [oauthAuthUrl, setOauthAuthUrl] = useState("");
  const [oauthTokenUrl, setOauthTokenUrl] = useState("");

  const [transportTab, setTransportTab] = useState<TransportTab>("stdio");
  const [expandedServer, setExpandedServer] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");

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
    setFormName("");
    setFormPrefix("");
    setFormScope("global");
    setFormCommand("");
    setFormArgs("");
    setEnvVars([]);
    setFormUrl("");
    setAuthType("none");
    setBearerToken("");
    setOauthClientId("");
    setOauthClientSecret("");
    setOauthScopes("");
    setOauthAuthUrl("");
    setOauthTokenUrl("");
    setTransportTab("stdio");
  };

  const handleAdd = async () => {
    setSaveError("");
    setSaving(true);

    const body: Record<string, unknown> = {
      name: formName,
      tool_name_prefix: formPrefix || undefined,
      scope: formScope,
    };

    if (transportTab === "stdio") {
      const env: Record<string, string> = {};
      envVars.forEach((ev) => {
        if (ev.key) env[ev.key] = ev.value;
      });
      const args = formArgs
        ? formArgs.split(",").map((a) => a.trim()).filter(Boolean)
        : undefined;
      body.command = formCommand || undefined;
      body.args = args;
      body.env = Object.keys(env).length > 0 ? env : undefined;
    } else {
      body.url = formUrl || undefined;
      if (authType === "bearer") {
        body.auth = { type: "bearer", token: bearerToken };
      } else if (authType === "oauth2") {
        const oauth: Record<string, unknown> = { type: "oauth2", client_id: oauthClientId };
        if (oauthClientSecret) oauth.client_secret = oauthClientSecret;
        if (oauthScopes) {
          oauth.scopes = oauthScopes.split(",").map((s) => s.trim()).filter(Boolean);
        }
        if (oauthAuthUrl) oauth.auth_url = oauthAuthUrl;
        if (oauthTokenUrl) oauth.token_url = oauthTokenUrl;
        body.auth = oauth;
      }
    }

    try {
      const res = await fetch(`${serverUrl}/mcp`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      if (res.ok) {
        setShowAddForm(false);
        resetForm();
        fetchServers();
      } else {
        const text = await res.text().catch(() => "");
        setSaveError(text || `Request failed (${res.status})`);
      }
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : "Network error");
    } finally {
      setSaving(false);
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

  const tabCls = (active: boolean) =>
    `flex-1 text-center px-3 py-1.5 rounded text-xs font-medium cursor-pointer transition-colors ${
      active
        ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
        : "bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80"
    }`;

  const authRadioCls = (selected: boolean) =>
    `flex items-center gap-1.5 px-2.5 py-1.5 rounded text-xs font-medium cursor-pointer transition-colors ${
      selected
        ? "bg-blue-500/20 text-blue-400 border border-blue-500/30"
        : "bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80"
    }`;

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
              <div key={srv.name} className="settings-card">
                <div
                  className="flex items-center justify-between gap-3 cursor-pointer"
                  onClick={() => setExpandedServer(expandedServer === srv.name ? null : srv.name)}
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <ChevronRight
                      className={`w-3.5 h-3.5 flex-shrink-0 text-[var(--text-muted)] transition-transform ${
                        expandedServer === srv.name ? "rotate-90" : ""
                      }`}
                    />
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
                  <div className="flex items-center gap-1.5 flex-shrink-0" onClick={(e) => e.stopPropagation()}>
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

                {expandedServer === srv.name && (
                  <div className="mt-3 pt-3 border-t border-[var(--border-base)] space-y-3">
                    {srv.tool_names.length > 0 && (
                      <div>
                        <div className="text-xs font-medium text-[var(--text-secondary)] mb-1.5">
                          Tools ({srv.tool_count})
                        </div>
                        <div className="flex flex-col gap-1.5">
                          {srv.tool_names.map((tool, i) => (
                            <div
                              key={tool}
                              className="px-2.5 py-1.5 rounded text-[11px] bg-blue-500/10 text-blue-400 border border-blue-500/20"
                            >
                              <div className="font-medium">{tool}</div>
                              {srv.tool_descriptions[i] && (
                                <div className="text-[10px] text-blue-300/70 mt-0.5 leading-tight">
                                  {srv.tool_descriptions[i]}
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    {srv.prompt_names.length > 0 && (
                      <div>
                        <div className="text-xs font-medium text-[var(--text-secondary)] mb-1.5">
                          Prompts ({srv.prompt_count})
                        </div>
                        <div className="flex flex-col gap-1.5">
                          {srv.prompt_names.map((prompt, i) => (
                            <div
                              key={prompt}
                              className="px-2.5 py-1.5 rounded text-[11px] bg-purple-500/10 text-purple-400 border border-purple-500/20"
                            >
                              <div className="font-medium">{prompt}</div>
                              {srv.prompt_descriptions[i] && (
                                <div className="text-[10px] text-purple-300/70 mt-0.5 leading-tight">
                                  {srv.prompt_descriptions[i]}
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    {srv.resource_names.length > 0 && (
                      <div>
                        <div className="text-xs font-medium text-[var(--text-secondary)] mb-1.5">
                          Resources ({srv.resource_count})
                        </div>
                        <div className="flex flex-col gap-1.5">
                          {srv.resource_names.map((resource, i) => (
                            <div
                              key={resource}
                              className="px-2.5 py-1.5 rounded text-[11px] bg-green-500/10 text-green-400 border border-green-500/20"
                            >
                              <div className="font-medium">{resource}</div>
                              {srv.resource_descriptions[i] && (
                                <div className="text-[10px] text-green-300/70 mt-0.5 leading-tight">
                                  {srv.resource_descriptions[i]}
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                    {srv.tool_names.length === 0 && srv.prompt_names.length === 0 && srv.resource_names.length === 0 && (
                      <div className="text-[11px] text-[var(--text-muted)] italic">
                        No tools, prompts, or resources available.
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Delete Confirm Dialog */}
      {deleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]">
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
        <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]">
          <div className="bg-[var(--surface-secondary)] border border-[var(--border-base)] rounded-lg p-5 max-w-lg w-full mx-3 shadow-xl max-h-[85vh] overflow-y-auto">
            <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-4">Add MCP Server</h3>

            <div className="space-y-3">
              {/* Name */}
              <div>
                <label className={labelCls}>Name</label>
                <input
                  className={inputCls}
                  placeholder="my-server"
                  value={formName}
                  onChange={(e) => setFormName(e.target.value)}
                />
              </div>

              {/* Transport Tabs */}
              <div>
                <label className={labelCls}>Transport</label>
                <div className="flex gap-2 mt-1">
                  <div className={tabCls(transportTab === "stdio")} onClick={() => setTransportTab("stdio")}>
                    Local (stdio)
                  </div>
                  <div className={tabCls(transportTab === "http")} onClick={() => setTransportTab("http")}>
                    HTTP/SSE
                  </div>
                </div>
              </div>

              {/* Stdio Fields */}
              {transportTab === "stdio" && (
                <>
                  <div>
                    <label className={labelCls}>Command</label>
                    <input
                      className={inputCls}
                      placeholder="npx"
                      value={formCommand}
                      onChange={(e) => setFormCommand(e.target.value)}
                    />
                  </div>
                  <div>
                    <label className={labelCls}>Args <span className="text-[var(--text-muted)]">(comma-separated)</span></label>
                    <input
                      className={inputCls}
                      placeholder="-y, @modelcontextprotocol/server-filesystem, ."
                      value={formArgs}
                      onChange={(e) => setFormArgs(e.target.value)}
                    />
                  </div>
                  <div>
                    <label className={labelCls}>Environment Variables</label>
                    <div className="space-y-1.5 mt-1">
                      {envVars.map((ev, i) => (
                        <div key={i} className="flex items-center gap-1.5">
                          <input
                            className={`${inputCls} !w-[60px]`}
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
                </>
              )}

              {/* HTTP Fields */}
              {transportTab === "http" && (
                <>
                  <div>
                    <label className={labelCls}>URL</label>
                    <input
                      className={inputCls}
                      placeholder="https://api.example.com/mcp"
                      value={formUrl}
                      onChange={(e) => setFormUrl(e.target.value)}
                    />
                  </div>

                  {/* Auth Type */}
                  <div>
                    <label className={labelCls}>Authentication</label>
                    <div className="flex gap-2 mt-1">
                      {(["none", "bearer", "oauth2"] as const).map((t) => (
                        <div
                          key={t}
                          className={authRadioCls(authType === t)}
                          onClick={() => setAuthType(t)}
                        >
                          {t === "none" ? "None" : t === "bearer" ? "Bearer" : "OAuth2"}
                        </div>
                      ))}
                    </div>
                  </div>

                  {/* Bearer Token */}
                  {authType === "bearer" && (
                    <div>
                      <label className={labelCls}>Token</label>
                      <input
                        className={inputCls}
                        placeholder="sk-your-token"
                        value={bearerToken}
                        onChange={(e) => setBearerToken(e.target.value)}
                      />
                    </div>
                  )}

                  {/* OAuth2 */}
                  {authType === "oauth2" && (
                    <>
                      <div>
                        <label className={labelCls}>Client ID</label>
                        <input
                          className={inputCls}
                          placeholder="your-client-id"
                          value={oauthClientId}
                          onChange={(e) => setOauthClientId(e.target.value)}
                        />
                      </div>
                      <div>
                        <label className={labelCls}>Client Secret</label>
                        <input
                          className={inputCls}
                          placeholder="optional"
                          value={oauthClientSecret}
                          onChange={(e) => setOauthClientSecret(e.target.value)}
                        />
                      </div>
                      <div>
                        <label className={labelCls}>Scopes <span className="text-[var(--text-muted)]">(comma-separated)</span></label>
                        <input
                          className={inputCls}
                          placeholder="repo, user"
                          value={oauthScopes}
                          onChange={(e) => setOauthScopes(e.target.value)}
                        />
                      </div>
                      <div>
                        <label className={labelCls}>Auth URL</label>
                        <input
                          className={inputCls}
                          placeholder="https://auth.example.com/authorize"
                          value={oauthAuthUrl}
                          onChange={(e) => setOauthAuthUrl(e.target.value)}
                        />
                      </div>
                      <div>
                        <label className={labelCls}>Token URL</label>
                        <input
                          className={inputCls}
                          placeholder="https://auth.example.com/token"
                          value={oauthTokenUrl}
                          onChange={(e) => setOauthTokenUrl(e.target.value)}
                        />
                      </div>
                    </>
                  )}
                </>
              )}

              {/* Tool Name Prefix */}
              <div>
                <label className={labelCls}>Tool Name Prefix</label>
                <input
                  className={inputCls}
                  placeholder="my_"
                  value={formPrefix}
                  onChange={(e) => setFormPrefix(e.target.value)}
                />
              </div>

              {/* Scope */}
              <div>
                <label className={labelCls}>Save to</label>
                <div className="flex gap-2 mt-1">
                  {(["global", "project"] as const).map((s) => (
                    <div
                      key={s}
                      className={authRadioCls(formScope === s)}
                      onClick={() => setFormScope(s)}
                    >
                      {s === "global" ? "Global (~/.pick/)" : "Project (.pick/)"}
                    </div>
                  ))}
                </div>
              </div>
            </div>

            {saveError && (
              <div className="text-xs text-red-400 break-words mt-5">{saveError}</div>
            )}
            <div className="flex justify-end gap-2 mt-3">
              <button
                onClick={() => { setShowAddForm(false); resetForm(); }}
                className="px-3 py-1.5 rounded text-xs font-medium bg-[var(--surface-button)] text-[var(--text-secondary)] border border-[var(--border-base)] hover:opacity-80 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAdd}
                disabled={!formName || saving}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium bg-blue-500/20 text-blue-400 border border-blue-500/30 hover:opacity-80 transition-colors disabled:opacity-40"
              >
                {saving ? (
                  <><Loader2 className="w-3 h-3 animate-spin" /> Saving...</>
                ) : "Save"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
