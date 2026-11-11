"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n";
import {
  DEFAULT_RENAME_FORMATS,
  type RenameFormatSettings,
} from "@/lib/settingsSections";
import { useSettingsSection, type SettingsSectionHandle } from "../SettingsDraftProvider";

const AVAILABLE_VARS = [
  { name: "series_name", example: "Dragon Ball" },
  { name: "volume", example: "1" },
  { name: "volume_padded", example: "01" },
  { name: "title", example: "Son Goku et ses amis" },
  { name: "authors", example: "Akira Toriyama" },
  { name: "publish_date", example: "2003-03-04" },
  { name: "isbn", example: "978-2723434546" },
];

type TemplateKey = "regular" | "hs" | "int" | "oneshot";

const TEMPLATES: {
  key: TemplateKey;
  settingKey: keyof RenameFormatSettings;
  labelKey: TranslationKey;
  previewKey: TranslationKey;
}[] = [
  {
    key: "regular",
    settingKey: "rename_format",
    labelKey: "rename.template",
    previewKey: "rename.examplePreview",
  },
  {
    key: "hs",
    settingKey: "rename_format_hs",
    labelKey: "rename.templateHs",
    previewKey: "rename.examplePreviewHs",
  },
  {
    key: "int",
    settingKey: "rename_format_int",
    labelKey: "rename.templateInt",
    previewKey: "rename.examplePreviewInt",
  },
  {
    key: "oneshot",
    settingKey: "rename_format_oneshot",
    labelKey: "rename.templateOneshot",
    previewKey: "rename.examplePreviewOneshot",
  },
];

function applyExampleTemplate(template: string): string {
  let result = template;
  for (const v of AVAILABLE_VARS) {
    result = result.replaceAll(`{${v.name}}`, v.example);
  }
  // Clean up dangling separators for missing vars
  result = result.replace(/\s*-\s*\{[^}]+\}/g, "");
  result = result.replace(/\{[^}]+\}\s*-\s*/g, "");
  result = result.replace(/\{[^}]+\}/g, "");
  return result.trim() + ".cbz";
}

/**
 * Rename templates are four independent scalar settings, not one object, so
 * each row writes its own key through the shared draft.
 */
export function RenameFormatCard() {
  const { t } = useTranslation();
  const regular = useSettingsSection<string>("rename_format");
  const hs = useSettingsSection<string>("rename_format_hs");
  const int = useSettingsSection<string>("rename_format_int");
  const oneshot = useSettingsSection<string>("rename_format_oneshot");
  const sections: Record<TemplateKey, SettingsSectionHandle<string>> = {
    regular,
    hs,
    int,
    oneshot,
  };
  const [activeField, setActiveField] = useState<TemplateKey>("regular");

  /** An explicit empty saved value keeps displaying the built-in default. */
  function displayValue(tpl: (typeof TEMPLATES)[number]): string {
    const stored = sections[tpl.key].value;
    return stored === "" ? DEFAULT_RENAME_FORMATS[tpl.settingKey] : stored;
  }

  const appendVar = (name: string) => {
    const tpl = TEMPLATES.find((item) => item.key === activeField)!;
    sections[activeField].set(`${displayValue(tpl)}{${name}}`);
  };

  return (
    <SettingsCard icon="edit" title={t("rename.settingTitle")} description={t("rename.settingDesc")}>
      {TEMPLATES.map((tpl) => {
        const section = sections[tpl.key];
        const value = displayValue(tpl);
        return (
          <SettingsRow
            key={tpl.key}
            label={t(tpl.labelKey)}
            htmlFor={`rename-format-${tpl.key}`}
            controlClassName="sm:max-w-lg"
          >
            <FormInput
              id={`rename-format-${tpl.key}`}
              value={value}
              onFocus={() => setActiveField(tpl.key)}
              onChange={(e) => section.set(e.target.value)}
              onBlur={() => void section.save()}
              placeholder={DEFAULT_RENAME_FORMATS[tpl.settingKey]}
            />

            <div className="settings-well mt-2 rounded-xl px-3.5 py-2.5">
              <p className="mb-1.5 text-xs font-medium text-muted-foreground">{t(tpl.previewKey)}</p>
              <p className="truncate font-mono text-sm text-foreground" title={applyExampleTemplate(value)}>{applyExampleTemplate(value)}</p>
            </div>
          </SettingsRow>
        );
      })}

      <div className="pt-2">
        <p className="mb-2 text-xs font-medium text-muted-foreground">{t("rename.availableVars")}</p>
        <div className="flex flex-wrap gap-2">
          {AVAILABLE_VARS.map((v) => (
            <code
              key={v.name}
              className="settings-well cursor-pointer rounded-full px-2.5 py-1 font-mono text-xs text-muted-foreground transition-colors hover:text-foreground"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => appendVar(v.name)}
              title={v.example}
            >
              {`{${v.name}}`}
            </code>
          ))}
        </div>
      </div>
    </SettingsCard>
  );
}
