import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DdlEngineCard, DDL_ENGINE_RANGES } from "@/app/(app)/settings/components/DdlEngineCard";
import { renderWithSettingsDraft } from "../../helpers/settingsDraft";

function field(key: keyof typeof DDL_ENGINE_RANGES) {
  return document.getElementById(`ddl-engine-${key}`) as HTMLInputElement;
}

function renderCard(limits: Record<string, unknown> = {}) {
  return renderWithSettingsDraft(<DdlEngineCard />, { initial: { limits } });
}

describe("DdlEngineCard", () => {
  it("renders the server defaults when limits is empty", () => {
    renderCard();

    expect(screen.getByText("ddlEngine.title")).toBeInTheDocument();
    for (const key of Object.keys(DDL_ENGINE_RANGES) as (keyof typeof DDL_ENGINE_RANGES)[]) {
      expect(field(key)).toHaveValue(DDL_ENGINE_RANGES[key].default);
    }
  });

  it("prefills every field from the stored limits", () => {
    renderCard({
      concurrent_ddl_provider_downloads: 6,
      ddl_download_connections: 9,
      download_max_attempts: 12,
      download_stall_timeout_seconds: 240,
      download_timeout_minutes: 90,
    });

    expect(field("concurrent_ddl_provider_downloads")).toHaveValue(6);
    expect(field("ddl_download_connections")).toHaveValue(9);
    expect(field("download_max_attempts")).toHaveValue(12);
    expect(field("download_stall_timeout_seconds")).toHaveValue(240);
    expect(field("download_timeout_minutes")).toHaveValue(90);
  });

  it("saves a single field on blur without touching the others", async () => {
    const { writer } = renderCard();

    fireEvent.change(field("ddl_download_connections"), { target: { value: "8" } });
    fireEvent.blur(field("ddl_download_connections"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "limits",
      expect.objectContaining({ ddl_download_connections: 8 }),
    ));
  });

  it("clamps values into the server range before saving", async () => {
    const { writer } = renderCard();

    fireEvent.change(field("download_max_attempts"), { target: { value: "9999" } });
    fireEvent.blur(field("download_max_attempts"));
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "limits",
      expect.objectContaining({
        download_max_attempts: DDL_ENGINE_RANGES.download_max_attempts.max,
      }),
    ));

    // A 0 must never reach the API (it would build a zero-length timeout).
    fireEvent.change(field("download_stall_timeout_seconds"), { target: { value: "0" } });
    fireEvent.blur(field("download_stall_timeout_seconds"));
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "limits",
      expect.objectContaining({
        download_stall_timeout_seconds: DDL_ENGINE_RANGES.download_stall_timeout_seconds.min,
      }),
    ));
  });

  it("falls back to the default when the field is emptied", async () => {
    const { writer } = renderCard({ download_timeout_minutes: 45 });

    fireEvent.change(field("download_timeout_minutes"), { target: { value: "" } });
    fireEvent.blur(field("download_timeout_minutes"));

    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "limits",
      expect.objectContaining({
        download_timeout_minutes: DDL_ENGINE_RANGES.download_timeout_minutes.default,
      }),
    ));
  });
});
