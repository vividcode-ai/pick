import { useState, useEffect, useCallback } from "react";
import { ChevronRight, File, Folder, FolderOpen } from "lucide-react";

interface FileEntry {
  name: string;
  type: string;
  size: number | null;
  modified: number | null;
}

interface FileTreeProps {
  baseUrl: string;
  rootPath?: string;
  onFileSelect: (path: string) => void;
  selectedFile?: string | null;
}

interface TreeNode {
  name: string;
  path: string;
  type: "file" | "dir";
  children?: TreeNode[];
  expanded?: boolean;
  loaded?: boolean;
}

function getFileIcon(name: string) {
  const ext = name.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "ts":
    case "tsx": return "📘";
    case "js":
    case "jsx": return "📙";
    case "css":
    case "scss": return "🎨";
    case "json": return "📋";
    case "md": return "📝";
    case "html": return "🌐";
    case "rs": return "🦀";
    case "py": return "🐍";
    case "go": return "🔵";
    default: return "📄";
  }
}

function TreeNodeItem({
  node,
  depth,
  baseUrl,
  selectedFile,
  onFileSelect,
  onToggle,
  getNode,
}: {
  node: TreeNode;
  depth: number;
  baseUrl: string;
  selectedFile?: string | null;
  onFileSelect: (path: string) => void;
  onToggle: (path: string) => void;
  getNode: (path: string) => TreeNode | undefined;
}) {
  const resolved = getNode(node.path) ?? node;
  const isDir = resolved.type === "dir";
  const isExpanded = resolved.expanded ?? false;
  const isLoaded = resolved.loaded ?? false;
  const children = resolved.children;
  const isSelected = selectedFile === node.path;

  return (
    <div>
      <div
        className={`flex items-center gap-1 py-[2px] px-2 text-xs cursor-pointer rounded-sm transition-colors ${
          isSelected
            ? "bg-[var(--accent-primary)]/10 text-[var(--accent-primary)]"
            : "hover:bg-[var(--surface-hover)] text-[var(--text-primary)]"
        }`}
        style={{ paddingLeft: `${8 + depth * 16}px` }}
        onClick={() => {
          if (isDir) {
            onToggle(node.path);
          } else {
            onFileSelect(node.path);
          }
        }}
      >
        {isDir ? (
          <span className="shrink-0 w-4 h-4 flex items-center justify-center">
            {isExpanded
              ? <ChevronRight className="w-3 h-3 rotate-90 transition-transform text-[var(--text-muted)]" />
              : <ChevronRight className="w-3 h-3 transition-transform text-[var(--text-muted)]" />
            }
          </span>
        ) : (
          <span className="w-4 shrink-0" />
        )}
        <span className="shrink-0 text-xs">
          {isDir
            ? (isExpanded ? <FolderOpen className="w-3.5 h-3.5 text-amber-400" /> : <Folder className="w-3.5 h-3.5 text-amber-400" />)
            : <span className="text-xs">{getFileIcon(node.name)}</span>
          }
        </span>
        <span className="whitespace-nowrap">{node.name}</span>
        {isDir && !isLoaded && (
          <span className="text-[var(--text-muted)] text-[10px] ml-auto">...</span>
        )}
      </div>
      {isDir && isExpanded && children && (
        <div>
          {children.map((child) => (
            <TreeNodeItem
              key={child.path}
              node={child}
              depth={depth + 1}
              baseUrl={baseUrl}
              selectedFile={selectedFile}
              onFileSelect={onFileSelect}
              onToggle={onToggle}
              getNode={getNode}
            />
          ))}
          {children.length === 0 && (
            <div
              className="text-[var(--text-muted)] text-[10px] italic px-2 py-1"
              style={{ paddingLeft: `${8 + (depth + 1) * 16}px` }}
            >
              (empty)
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function FileTree({ baseUrl, rootPath = ".", onFileSelect, selectedFile }: FileTreeProps) {
  const [tree, setTree] = useState<Record<string, TreeNode>>(() => ({
    [rootPath]: { name: "", path: rootPath, type: "dir", children: [], expanded: false, loaded: false },
  }));

  const fetchChildren = useCallback(async (dirPath: string) => {
    try {
      const res = await fetch(`${baseUrl}/files/list?path=${encodeURIComponent(dirPath)}&limit=500`);
      if (!res.ok) return;
      const data = await res.json();
      const entries: FileEntry[] = data.entries || [];
      const children: TreeNode[] = entries
        .filter((e) => e.type === "directory" || e.type === "file")
        .map((e) => ({
          name: e.name,
          path: dirPath === "." ? e.name : `${dirPath}/${e.name}`,
          type: e.type === "directory" ? "dir" : "file",
        }));

      setTree((prev) => {
        const updated = { ...prev, [dirPath]: { ...prev[dirPath], children, loaded: true, expanded: true } };
        for (const child of children) {
          if (child.type === "dir") {
            updated[child.path] = child;
          }
        }
        return updated;
      });
    } catch (e) {
      console.error("Failed to list directory:", e);
    }
  }, [baseUrl]);

  useEffect(() => {
    const root = tree[rootPath];
    if (!root?.loaded) {
      fetchChildren(rootPath);
    }
  }, [rootPath, fetchChildren, tree]);

  const handleToggle = useCallback((path: string) => {
    const node = tree[path];
    if (!node) return;
    if (node.expanded) {
      setTree((prev) => ({
        ...prev,
        [path]: { ...prev[path], expanded: false },
      }));
    } else {
      if (!node.loaded) {
        fetchChildren(path);
      } else {
        setTree((prev) => ({
          ...prev,
          [path]: { ...prev[path], expanded: true },
        }));
      }
    }
  }, [tree, fetchChildren]);

  const root = tree[rootPath];
  if (!root) return null;

  const showEmpty = root.loaded && root.children && root.children.length === 0;

  return (
    <div className="select-none">
      {showEmpty ? (
        <div className="text-[var(--text-muted)] text-[10px] italic px-3 py-2">
          (empty)
        </div>
      ) : (
        root.children?.map((child) => (
          <TreeNodeItem
            key={child.path}
            node={child}
            depth={0}
            baseUrl={baseUrl}
            selectedFile={selectedFile}
            onFileSelect={onFileSelect}
            onToggle={handleToggle}
            getNode={(path) => tree[path]}
          />
        ))
      )}
    </div>
  );
}
