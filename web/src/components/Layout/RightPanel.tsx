import { useState, type ReactNode } from "react";
import { ChangesTab } from "./right-panel/ChangesTab";
import { FilesTab } from "./right-panel/FilesTab";
import { GitChangesTab } from "./right-panel/GitChangesTab";
import { StatusTab } from "./right-panel/StatusTab";

interface RightPanelProps {
  diffs?: { filePath: string; content: string }[];
  connected: boolean;
}

type TabId = "changes" | "files" | "git" | "status";

const tabs: { id: TabId; label: string }[] = [
  { id: "changes", label: "Changes" },
  { id: "files", label: "Files" },
  { id: "git", label: "Git" },
  { id: "status", label: "Status" },
];

export function RightPanel({ diffs, connected }: RightPanelProps) {
  const [activeTab, setActiveTab] = useState<TabId>("changes");

  const tabContent: Record<TabId, ReactNode> = {
    changes: <ChangesTab diffs={diffs} />,
    files: <FilesTab />,
    git: <GitChangesTab />,
    status: <StatusTab connected={connected} />,
  };

  return (
    <div className="right-panel-container panel">
      <div className="right-panel-tabs">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className="right-panel-tab"
            data-active={activeTab === tab.id}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="panel-body">
        {tabContent[activeTab]}
      </div>
    </div>
  );
}
