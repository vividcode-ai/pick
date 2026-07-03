import { useSyncExternalStore } from "react";
import { ArrowLeft, Palette, Cpu, Server, Bell, Archive } from "lucide-react";
import {
  closeSettings,
  setActiveSettingsSection,
  subscribeToSettings,
  getSettingsSnapshot,
  type SettingsSectionId,
} from "../../stores/settings";
import { useArchivedSessions } from "../../stores/sessions";
import { AppearanceSection } from "./AppearanceSection";
import { ProvidersSection } from "./ProvidersSection";
import { ServerSection } from "./ServerSection";
import { NotificationsSection } from "./NotificationsSection";
import { ArchivedSessionsSection } from "./ArchivedSectionsSection";
import type { ProviderInfo } from "../../types/events";

interface SettingsScreenProps {
  providers: ProviderInfo[];
  selectedModel: string;
  onModelChange: (modelId: string, provider: string) => void;
  serverUrl: string;
  onSaveServerUrl: (url: string) => void;
  onUnarchiveSession: (id: string) => void;
  onDeleteArchivedSession: (id: string) => void;
}

const navItems: { id: SettingsSectionId; icon: typeof Palette; label: string }[] = [
  { id: "appearance", icon: Palette, label: "Appearance" },
  { id: "providers", icon: Cpu, label: "Providers" },
  { id: "server", icon: Server, label: "Server" },
  { id: "notifications", icon: Bell, label: "Notifications" },
  { id: "archived", icon: Archive, label: "Archived" },
];

export function SettingsScreen({
  providers,
  selectedModel,
  onModelChange,
  serverUrl,
  onSaveServerUrl,
  onUnarchiveSession,
  onDeleteArchivedSession,
}: SettingsScreenProps) {
  const state = useSyncExternalStore(subscribeToSettings, getSettingsSnapshot, getSettingsSnapshot);
  const archivedSessions = useArchivedSessions();

  const renderSection = () => {
    switch (state.activeSection) {
      case "appearance":
        return <AppearanceSection />;
      case "providers":
        return <ProvidersSection providers={providers} selectedModel={selectedModel} onModelChange={onModelChange} />;
      case "server":
        return <ServerSection currentUrl={serverUrl} onSave={onSaveServerUrl} />;
      case "notifications":
        return <NotificationsSection />;
      case "archived":
        return (
          <ArchivedSessionsSection
            sessions={archivedSessions}
            onUnarchive={onUnarchiveSession}
            onDelete={onDeleteArchivedSession}
          />
        );
    }
  };

  return (
    <div className="settings-page">
      <div className="settings-shell">
        <nav className="settings-nav">
          <div className="settings-nav-header">
            <div className="flex flex-col gap-2">
              <button
                onClick={closeSettings}
                className="flex items-center gap-1.5 px-2 py-1.5 rounded-md bg-neutral-800 hover:bg-neutral-700 text-neutral-200 hover:text-white border border-neutral-700 transition-colors self-start"
                aria-label="Back to main"
              >
                <ArrowLeft className="w-4 h-4" />
                <span className="text-xs">Back to main</span>
              </button>
              <h2 className="settings-nav-title">Settings</h2>
            </div>
          </div>
          <div className="settings-nav-list">
            {navItems.map((item) => {
              const Icon = item.icon;
              return (
                <button
                  key={item.id}
                  className="settings-nav-button"
                  data-selected={state.activeSection === item.id}
                  onClick={() => setActiveSettingsSection(item.id)}
                >
                  <Icon className="w-4 h-4" />
                  <span>{item.label}</span>
                </button>
              );
            })}
          </div>
          <div className="flex-1" />
          <div className="text-[10px] text-neutral-500 pt-3 border-t border-neutral-800">
            Pick v0.1.0
          </div>
        </nav>

        <div className="settings-content">
          <header className="settings-content-header">
            <h1 className="settings-content-title">
              {navItems.find((i) => i.id === state.activeSection)?.label}
            </h1>
          </header>
          <div className="settings-scroll">
            <div className="settings-content-card">
              {renderSection()}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
