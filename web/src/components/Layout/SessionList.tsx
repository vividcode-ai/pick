import { useState, useEffect, useMemo, useCallback } from "react";
import { useSessionSearch } from "../../stores/sessions";
import { useProjectsState } from "../../stores/projects";
import { SessionSearch } from "./SessionSearch";
import { SessionItem } from "./SessionItem";
import { ProjectGroup } from "../Project/ProjectGroup";

interface SessionListProps {
  activeSessionId: string | null;
  onSelectSession: (id: string) => void;
  onNewSession: () => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  streamingSessions?: Record<string, boolean>;
  isTauri?: boolean;
  onProjectSwitch?: (path: string) => void;
  onDeleteProject?: (path: string) => void;
}

export function SessionList({
  activeSessionId,
  onSelectSession,
  onNewSession,
  onRenameSession,
  onArchiveSession,
  streamingSessions,
  isTauri,
  onProjectSwitch,
  onDeleteProject,
}: SessionListProps) {
  const { query, setQuery, filtered } = useSessionSearch();
  const { projects, currentCwd } = useProjectsState();
  const [selectedProject, setSelectedProject] = useState<string | null>(null);

  // Auto-select current project when it changes
  useEffect(() => {
    if (isTauri && currentCwd) {
      setSelectedProject(currentCwd);
    }
  }, [isTauri, currentCwd]);

  const projectList = useMemo(() => {
    if (!isTauri) return [];

    // Collect unique project cwds from sessions
    const cwds = new Set<string>();
    for (const s of filtered) {
      const key = s.cwd || "__default__";
      cwds.add(key);
    }
    // Merge in all projects from projects.json
    for (const p of projects) {
      cwds.add(p.path);
    }

    return Array.from(cwds)
      .map((cwd) => ({
        cwd,
        name: cwd === "__default__" ? "Other" : cwd.split(/[\\/]/).pop() || cwd,
        sessions: filtered.filter((s) => (s.cwd || "__default__") === cwd),
        isCurrent: currentCwd === cwd,
      }))
      .sort((a, b) => {
        if (a.cwd === "__default__") return 1;
        if (b.cwd === "__default__") return -1;
        return a.name.localeCompare(b.name);
      });
  }, [filtered, isTauri, projects, currentCwd]);

  const handleSelectProject = useCallback((cwd: string) => {
    setSelectedProject((prev) => (prev === cwd ? null : cwd));
    // Fire-and-forget: update the server's cwd marker so new sessions
    // are created under the correct project.  Visual selection is
    // immediate — no await, no blocking.
    if (onProjectSwitch && cwd !== currentCwd) {
      onProjectSwitch(cwd);
    }
  }, [onProjectSwitch, currentCwd]);

  if (isTauri) {
    return (
      <div className="flex flex-col min-h-0 flex-1">
        <SessionSearch query={query} onQueryChange={setQuery} />
        <div className="flex-1 overflow-y-auto px-2 py-1 space-y-0.5">
          {projectList.length === 0 ? (
            <div className="text-xs text-neutral-500 text-center py-8">
              {query ? "No matching sessions" : "No sessions yet"}
            </div>
          ) : (
            projectList.map((group) => (
              <ProjectGroup
                key={group.cwd}
                name={group.name}
                path={group.cwd}
                sessions={group.sessions}
                isSelected={selectedProject === group.cwd}
                activeSessionId={activeSessionId}
                onSelectSession={onSelectSession}
                onRenameSession={onRenameSession}
                onArchiveSession={onArchiveSession}
                streamingSessions={streamingSessions}
                onSelect={() => handleSelectProject(group.cwd)}
                onDelete={onDeleteProject}
              />
            ))
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <SessionSearch query={query} onQueryChange={setQuery} />
      <div className="flex-1 overflow-y-auto px-2 py-1 space-y-0.5">
        {filtered.length === 0 ? (
          <div className="text-xs text-neutral-500 text-center py-8">
            {query ? "No matching sessions" : "No sessions yet"}
          </div>
        ) : (
          filtered.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              isActive={session.id === activeSessionId}
              streaming={!!streamingSessions?.[session.id]}
              onSelect={onSelectSession}
              onRename={onRenameSession}
              onArchive={onArchiveSession}
            />
          ))
        )}
      </div>
    </div>
  );
}
