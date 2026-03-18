"use client";

import { useState, useCallback } from "react";
import { FolderItem } from "../../lib/api";
import { useTranslation } from "../../lib/i18n/context";

interface TreeNode extends FolderItem {
  children?: TreeNode[];
  isLoading?: boolean;
}

interface FolderBrowserProps {
  initialFolders: FolderItem[];
  selectedPath: string;
  onSelect: (path: string) => void;
}

export function FolderBrowser({ initialFolders, selectedPath, onSelect }: FolderBrowserProps) {
  const { t } = useTranslation();
  // Convert initial folders to tree structure
  const [tree, setTree] = useState<TreeNode[]>(
    initialFolders.map(f => ({ ...f, children: f.has_children ? [] : undefined }))
  );
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set());

  const loadChildren = useCallback(async (parentPath: string): Promise<FolderItem[]> => {
    try {
      const response = await fetch(`/api/folders?path=${encodeURIComponent(parentPath)}`);
      if (response.ok) {
        return await response.json();
      }
    } catch (error) {
      console.error("Failed to load folders:", error);
    }
    return [];
  }, []);

  const findAndUpdateNode = (
    nodes: TreeNode[],
    targetPath: string,
    updateFn: (node: TreeNode) => TreeNode
  ): TreeNode[] => {
    return nodes.map(node => {
      if (node.path === targetPath) {
        return updateFn(node);
      }
      if (node.children) {
        return { ...node, children: findAndUpdateNode(node.children, targetPath, updateFn) };
      }
      return node;
    });
  };

  const toggleExpand = useCallback(async (node: TreeNode) => {
    if (!node.has_children) {
      onSelect(node.path);
      return;
    }

    const isExpanded = expandedPaths.has(node.path);
    
    if (isExpanded) {
      // Collapse
      setExpandedPaths(prev => {
        const next = new Set(prev);
        next.delete(node.path);
        return next;
      });
    } else {
      // Expand
      setExpandedPaths(prev => new Set(prev).add(node.path));
      
      // Load children if not already loaded
      if (!node.children || node.children.length === 0) {
        setTree(prev => findAndUpdateNode(prev, node.path, n => ({ ...n, isLoading: true })));
        
        const children = await loadChildren(node.path);
        const childNodes = children.map(f => ({ ...f, children: f.has_children ? [] : undefined }));
        
        setTree(prev => findAndUpdateNode(prev, node.path, n => ({ 
          ...n, 
          children: childNodes,
          isLoading: false 
        })));
      }
    }
  }, [expandedPaths, loadChildren, onSelect]);

  const renderNode = (node: TreeNode, level: number = 0) => {
    const isExpanded = expandedPaths.has(node.path);
    const isSelected = selectedPath === node.path;
    const hasChildren = node.has_children;
    
    return (
      <div key={node.path}>
        <div
          className={`flex items-center gap-2 px-3 py-2 hover:bg-accent/50 transition-colors cursor-pointer border-b border-border/10 last:border-b-0 ${
            isSelected ? 'bg-primary/10 hover:bg-primary/15' : ''
          }`}
          style={{ paddingLeft: `${12 + level * 20}px` }}
          onClick={() => onSelect(node.path)}
        >
          {/* Expand/Collapse button */}
          {hasChildren ? (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                toggleExpand(node);
              }}
              className="flex items-center justify-center w-5 h-5 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
            >
              {node.isLoading ? (
                <svg className="animate-spin w-3 h-3" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
              ) : (
                <svg
                  className={`w-3 h-3 transition-transform duration-200 ${isExpanded ? 'rotate-90' : ''}`}
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
              )}
            </button>
          ) : (
            <span className="w-5" />
          )}

          {/* Folder icon */}
          <svg
            className={`w-5 h-5 flex-shrink-0 ${isSelected ? 'text-primary' : 'text-warning'}`}
            fill={hasChildren ? "currentColor" : "none"}
            fillOpacity={hasChildren ? 0.2 : undefined}
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
            />
          </svg>

          {/* Folder name */}
          <span className={`flex-1 text-sm truncate ${isSelected ? 'font-medium text-primary' : 'text-foreground'}`}>
            {node.name}
          </span>

          {/* Selected indicator */}
          {isSelected && (
            <svg className="w-4 h-4 text-primary flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          )}
        </div>
        
        {/* Render children if expanded */}
        {isExpanded && node.children && node.children.length > 0 && (
          <div>
            {node.children.map(child => renderNode(child, level + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="bg-card overflow-hidden">
      {/* Folder tree */}
      <div className="max-h-80 overflow-y-auto">
        {tree.length === 0 ? (
          <div className="px-3 py-8 text-sm text-muted-foreground text-center">
            {t("folder.noFolders")}
          </div>
        ) : (
          tree.map(node => renderNode(node))
        )}
      </div>
    </div>
  );
}
