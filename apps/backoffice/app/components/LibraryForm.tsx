"use client";

import { useState } from "react";
import { FolderPicker } from "./FolderPicker";
import { FolderItem } from "../../lib/api";
import { Button, FormField, FormInput, FormRow } from "./ui";

interface LibraryFormProps {
  initialFolders: FolderItem[];
  action: (formData: FormData) => void;
}

export function LibraryForm({ initialFolders, action }: LibraryFormProps) {
  const [selectedPath, setSelectedPath] = useState<string>("");

  return (
    <form action={action}>
      <FormRow>
        <FormField className="flex-1 min-w-48">
          <FormInput name="name" placeholder="Library name" required />
        </FormField>
        <FormField className="flex-1 min-w-64">
          <input type="hidden" name="root_path" value={selectedPath} />
          <FolderPicker
            initialFolders={initialFolders}
            selectedPath={selectedPath}
            onSelect={setSelectedPath}
          />
        </FormField>
      </FormRow>
      <div className="mt-4 flex justify-end">
        <Button type="submit" disabled={!selectedPath}>
          Add Library
        </Button>
      </div>
    </form>
  );
}
