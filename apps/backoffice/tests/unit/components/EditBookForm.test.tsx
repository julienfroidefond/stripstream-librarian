import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { refresh } = vi.hoisted(() => ({ refresh: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh, push: vi.fn(), replace: vi.fn(), back: vi.fn(), prefetch: vi.fn() }),
}));

import { EditBookForm } from "@/app/components/EditBookForm";
import type { BookDto } from "@/lib/api";

function makeBook(overrides: Partial<BookDto> = {}): BookDto {
  return {
    id: "b1",
    library_id: "l1",
    kind: "cbz",
    format: "cbz",
    title: "Berserk vol. 1",
    author: "Kentaro Miura",
    authors: ["Kentaro Miura"],
    series: "Berserk",
    volume: 1,
    volume_type: "regular",
    language: "fr",
    page_count: 200,
    file_path: null,
    file_format: null,
    file_parse_status: null,
    updated_at: "2024-01-01",
    reading_status: "unread",
    reading_current_page: null,
    reading_last_read_at: null,
    series_id: "s1",
    summary: "A dark fantasy",
    isbn: "978",
    publish_date: "1989",
    locked_fields: {},
    ...overrides,
  };
}

function stubFetch(response: Partial<Response> = {}) {
  const fetchMock = vi.fn().mockResolvedValue({ ok: true, json: async () => ({}), ...response });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
  refresh.mockReset();
});

function openForm(book = makeBook()) {
  render(<EditBookForm book={book} />);
  fireEvent.click(screen.getByRole("button", { name: /editBook\.editMetadata/ }));
  return book;
}

describe("EditBookForm", () => {
  it("renders the trigger and opens a prefilled modal", () => {
    stubFetch();
    openForm();

    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("editBook.titlePlaceholder")).toHaveValue("Berserk vol. 1");
    expect(screen.getByPlaceholderText("editBook.seriesPlaceholder")).toHaveValue("Berserk");
    expect(screen.getByPlaceholderText("editBook.volumePlaceholder")).toHaveValue(1);
    expect(screen.getByPlaceholderText("editBook.languagePlaceholder")).toHaveValue("fr");
    expect(screen.getByPlaceholderText("ISBN")).toHaveValue("978");
    expect(screen.getByPlaceholderText("editBook.publishDatePlaceholder")).toHaveValue("1989");
    expect(screen.getByPlaceholderText("editBook.descriptionPlaceholder")).toHaveValue("A dark fantasy");
  });

  it("exposes an open callback to children", () => {
    stubFetch();
    render(
      <EditBookForm book={makeBook()}>
        {(open) => (
          <button type="button" onClick={open}>
            custom open
          </button>
        )}
      </EditBookForm>
    );

    expect(screen.queryByTestId("modal-panel")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "custom open" }));
    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
  });

  it("toggles the locked-fields legend", () => {
    stubFetch();
    openForm();

    expect(screen.queryByText("editBook.lockedFieldsNote")).toBeNull();
    fireEvent.click(screen.getAllByTitle("editBook.clickToLock")[0]);
    expect(screen.getByText("editBook.lockedFieldsNote")).toBeInTheDocument();
    fireEvent.click(screen.getByTitle("editBook.lockedField"));
    expect(screen.queryByText("editBook.lockedFieldsNote")).toBeNull();
  });

  it("adds and removes authors while ignoring duplicates", () => {
    stubFetch();
    openForm();

    const input = screen.getByPlaceholderText("editBook.addAuthor");
    fireEvent.change(input, { target: { value: "New Author" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByText("New Author")).toBeInTheDocument();

    fireEvent.change(input, { target: { value: "New Author" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getAllByText("New Author")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: 'editBook.removeAuthor:{"name":"New Author"}' }));
    expect(screen.queryByText("New Author")).toBeNull();
  });

  it("disables save when the title is empty", () => {
    stubFetch();
    openForm();

    fireEvent.change(screen.getByPlaceholderText("editBook.titlePlaceholder"), {
      target: { value: "   " },
    });

    expect(screen.getByRole("button", { name: "editBook.saveLabel" })).toBeDisabled();
  });

  it("saves the metadata and refreshes the router", async () => {
    const fetchMock = stubFetch();
    openForm();

    fireEvent.change(screen.getByPlaceholderText("editBook.titlePlaceholder"), {
      target: { value: "  New title  " },
    });
    fireEvent.change(screen.getByPlaceholderText("editBook.volumePlaceholder"), {
      target: { value: "7" },
    });
    fireEvent.submit(screen.getByPlaceholderText("editBook.titlePlaceholder").closest("form")!);

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/books/b1",
      expect.objectContaining({ method: "PATCH" })
    );
    const body = JSON.parse((fetchMock.mock.calls[0][1] as RequestInit).body as string);
    expect(body).toMatchObject({
      title: "New title",
      volume: 7,
      authors: ["Kentaro Miura"],
      author: "Kentaro Miura",
    });
    expect(screen.queryByTestId("modal-panel")).toBeNull();
  });

  it("shows the API error and stays open", async () => {
    stubFetch({ ok: false, json: async () => ({ error: "boom" }) });
    openForm();

    fireEvent.submit(screen.getByPlaceholderText("editBook.titlePlaceholder").closest("form")!);

    await waitFor(() => expect(screen.getByText("boom")).toBeInTheDocument());
    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
  });

  it("reports a network error", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    openForm();

    fireEvent.submit(screen.getByPlaceholderText("editBook.titlePlaceholder").closest("form")!);

    await waitFor(() => expect(screen.getByText("common.networkError")).toBeInTheDocument());
  });

  it("resets the fields when the modal is closed", () => {
    stubFetch();
    openForm();

    fireEvent.change(screen.getByPlaceholderText("editBook.titlePlaceholder"), {
      target: { value: "Changed" },
    });
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));
    expect(screen.queryByTestId("modal-panel")).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: /editBook\.editMetadata/ }));
    expect(screen.getByPlaceholderText("editBook.titlePlaceholder")).toHaveValue("Berserk vol. 1");
  });

  it("closes on Escape", () => {
    stubFetch();
    openForm();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByTestId("modal-panel")).toBeNull();
  });
});
