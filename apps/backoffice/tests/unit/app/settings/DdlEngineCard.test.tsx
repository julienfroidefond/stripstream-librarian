import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DdlEngineCard, DDL_ENGINE_RANGES } from "@/app/(app)/settings/components/DdlEngineCard";

function field(key: keyof typeof DDL_ENGINE_RANGES) {
  return document.getElementById(`ddl-engine-${key}`) as HTMLInputElement;
}

describe("DdlEngineCard", () => {
  it("renders the server defaults when limits is empty", () => {
    render(<DdlEngineCard limits={{}} onSave={vi.fn()} />);

    expect(screen.getByText("ddlEngine.title")).toBeInTheDocument();
    for (const key of Object.keys(DDL_ENGINE_RANGES) as (keyof typeof DDL_ENGINE_RANGES)[]) {
      expect(field(key)).toHaveValue(DDL_ENGINE_RANGES[key].default);
    }
  });

  it("prefills every field from the stored limits", () => {
    render(
      <DdlEngineCard
        limits={{
          concurrent_ddl_provider_downloads: 6,
          ddl_download_connections: 9,
          download_max_attempts: 12,
          download_stall_timeout_seconds: 240,
          download_timeout_minutes: 90,
        }}
        onSave={vi.fn()}
      />
    );

    expect(field("concurrent_ddl_provider_downloads")).toHaveValue(6);
    expect(field("ddl_download_connections")).toHaveValue(9);
    expect(field("download_max_attempts")).toHaveValue(12);
    expect(field("download_stall_timeout_seconds")).toHaveValue(240);
    expect(field("download_timeout_minutes")).toHaveValue(90);
  });

  it("saves a single field on blur without touching the others", () => {
    const onSave = vi.fn();
    render(<DdlEngineCard limits={{}} onSave={onSave} />);

    fireEvent.change(field("ddl_download_connections"), { target: { value: "8" } });
    fireEvent.blur(field("ddl_download_connections"));

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith({ ddl_download_connections: 8 });
  });

  it("clamps values into the server range before saving", () => {
    const onSave = vi.fn();
    render(<DdlEngineCard limits={{}} onSave={onSave} />);

    fireEvent.change(field("download_max_attempts"), { target: { value: "9999" } });
    fireEvent.blur(field("download_max_attempts"));
    expect(onSave).toHaveBeenLastCalledWith({
      download_max_attempts: DDL_ENGINE_RANGES.download_max_attempts.max,
    });

    // A 0 must never reach the API (it would build a zero-length timeout).
    fireEvent.change(field("download_stall_timeout_seconds"), { target: { value: "0" } });
    fireEvent.blur(field("download_stall_timeout_seconds"));
    expect(onSave).toHaveBeenLastCalledWith({
      download_stall_timeout_seconds: DDL_ENGINE_RANGES.download_stall_timeout_seconds.min,
    });
  });

  it("falls back to the default when the field is emptied", () => {
    const onSave = vi.fn();
    render(<DdlEngineCard limits={{ download_timeout_minutes: 45 }} onSave={onSave} />);

    fireEvent.change(field("download_timeout_minutes"), { target: { value: "" } });
    fireEvent.blur(field("download_timeout_minutes"));

    expect(onSave).toHaveBeenLastCalledWith({
      download_timeout_minutes: DDL_ENGINE_RANGES.download_timeout_minutes.default,
    });
  });
});
