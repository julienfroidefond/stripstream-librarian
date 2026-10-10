"use client";

import { useState } from "react";
import { FolderBrowser } from "./FolderBrowser";
import { FolderItem } from "../../lib/api";
import { Button, Icon, Modal } from "./ui";
import { useTranslation } from "@/lib/i18n/context";

interface FolderPickerProps {
  initialFolders: FolderItem[];
  selectedPath: string;
  onSelect: (path: string) => void;
}

export function FolderPicker({ initialFolders, selectedPath, onSelect }: FolderPickerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const { t } = useTranslation();

  const handleSelect = (path: string) => {
    onSelect(path);
    setIsOpen(false);
  };

  return (
    <div className="relative">
      {/* Input avec bouton browse */}
      <div className="flex items-center gap-2">
        <div className="flex-1 relative">
          <input
            type="text"
            readOnly
            value={selectedPath || t("folder.selectFolder")}
            className={`
              w-full px-3 py-2 rounded-lg border bg-card
              text-sm font-mono
              ${selectedPath ? 'text-foreground' : 'text-muted-foreground italic'}
              border-border/50 focus:border-primary/50 focus:ring-2 focus:ring-primary/20
              transition-all duration-200
            `}
          />
          {selectedPath && (
            <button
              type="button"
              onClick={() => onSelect("")}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-destructive transition-colors"
            >
              <Icon name="x" size="sm" />
            </button>
          )}
        </div>
        <Button
          type="button"
          variant="secondary"
          onClick={() => setIsOpen(true)}
          className="flex items-center gap-2"
        >
          <Icon name="folder" size="sm" />
          {t("common.browse")}
        </Button>
      </div>

      {/* Popup Modal */}
      <Modal
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        maxWidth="lg"
        title={
          <span className="flex items-center gap-2">
            <Icon name="folder" size="md" className="text-primary" />
            {t("folder.selectFolderTitle")}
          </span>
        }
        footer={
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground">
              {t("folder.clickToSelect")}
            </span>
            <div className="flex gap-2">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => setIsOpen(false)}
              >
                {t("common.cancel")}
              </Button>
            </div>
          </div>
        }
      >
        <FolderBrowser
          initialFolders={initialFolders}
          selectedPath={selectedPath}
          onSelect={handleSelect}
        />
      </Modal>
    </div>
  );
}
