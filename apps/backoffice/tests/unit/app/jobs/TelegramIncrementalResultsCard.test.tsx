import { render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it } from "vitest";

import { TelegramIncrementalResultsCard } from "@/app/(app)/jobs/[id]/components/TelegramIncrementalResultsCard";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

const t = ((key: string) => key) as unknown as TranslateFunction;

function renderCard(
  props: Partial<ComponentProps<typeof TelegramIncrementalResultsCard>> = {}
) {
  return render(
    <TelegramIncrementalResultsCard
      new_books={9}
      sources_scanned={3}
      sources={[]}
      recent_books={[]}
      t={t}
      {...props}
    />
  );
}

describe("TelegramIncrementalResultsCard", () => {
  it("reports unreadable sources without hiding the healthy ones", () => {
    const { container } = renderCard({
      sources: [
        { username: "healthy", new_books: 9, messages_scanned: 40 },
        {
          username: "broken",
          new_books: 0,
          messages_scanned: 0,
          error: "Channel @broken not found",
        },
      ],
    });

    const text = container.textContent ?? "";
    expect(text).toContain("jobDetail.telegramIncrementalPartial");
    expect(text).toContain("Channel @broken not found");
    expect(text).toContain("@healthy");
    expect(text).toContain("+9");
  });

  it("surfaces persistence errors that do not affect the job status", () => {
    const { container } = renderCard({
      sources: [
        { username: "a", new_books: 4, persistence_errors: 1 },
        { username: "b", new_books: 5, persistence_errors: 2 },
      ],
    });

    const text = container.textContent ?? "";
    // Aggregated once at the top...
    expect(
      text.match(/jobDetail\.telegramIncrementalPersistenceErrors/g)
    ).toHaveLength(3);
  });

  it("stays quiet when every source is healthy", () => {
    renderCard({
      sources: [
        { username: "a", new_books: 4, messages_scanned: 10 },
        { username: "b", new_books: 5, messages_scanned: 12 },
      ],
    });

    expect(
      screen.queryByText("jobDetail.telegramIncrementalPartial")
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("jobDetail.telegramIncrementalPersistenceErrors")
    ).not.toBeInTheDocument();
  });

  it("does not treat a legitimately empty channel as a failure", () => {
    renderCard({
      sources: [
        { username: "empty", new_books: 0, messages_scanned: 12 },
      ],
    });

    expect(
      screen.queryByText("jobDetail.telegramIncrementalPartial")
    ).not.toBeInTheDocument();
  });
});
