import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { refresh } = vi.hoisted(() => ({ refresh: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh, push: vi.fn(), replace: vi.fn(), back: vi.fn(), prefetch: vi.fn() }),
}));

import { EditBookForm } from "@/app/components/EditBookForm";
import { MetadataFieldsProvider } from "@/app/components/MetadataFieldsProvider";
import type { BookDetailsDto, BookItemDto } from "@/lib/api";
import { METADATA_FIELDS_CATALOG } from "../fixtures/metadataFields";

function makeBook(overrides: Partial<BookDetailsDto> = {}): BookDetailsDto {
  return {
    id: "b1",
    library_id: "l1",
    kind: "cbz",
    title: "Berserk vol. 1",
    author: "Kentaro Miura",
    authors: ["Kentaro Miura"],
    series: "Berserk",
    volume: 1,
    volume_type: "regular",
    language: "fr",
    page_count: 200,
    thumbnail_url: null,
    file_path: null,
    file_format: null,
    file_parse_status: null,
    file_size: null,
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
  render(
    <MetadataFieldsProvider catalog={METADATA_FIELDS_CATALOG}>
      <EditBookForm book={book} />
    </MetadataFieldsProvider>,
  );
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
      <MetadataFieldsProvider catalog={METADATA_FIELDS_CATALOG}>
        <EditBookForm book={makeBook()}>
          {(open) => (
            <button type="button" onClick={open}>
              custom open
            </button>
          )}
        </EditBookForm>
      </MetadataFieldsProvider>
    );

    expect(screen.queryByTestId("modal-panel")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "custom open" }));
    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
  });

  it("only offers locks for fields external sync can write", async () => {
    const fetchMock = stubFetch();
    openForm(
      makeBook({
        locked_fields: { title: true, series: true, volume: true, summary: true },
      })
    );

    // authors, language, isbn, publish_date and summary are the only lockable
    // book fields; title, series and volume locks are stale.
    expect(screen.getAllByTitle("editBook.clickToLock")).toHaveLength(4);
    expect(screen.getAllByTitle("editBook.lockedField")).toHaveLength(1);

    fireEvent.submit(screen.getByPlaceholderText("editBook.titlePlaceholder").closest("form")!);

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    const body = JSON.parse((fetchMock.mock.calls[0][1] as RequestInit).body as string);
    expect(body.locked_fields).toEqual({ summary: true });
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

  it("loads the detail of a list item before saving, keeping its ISBN and locks", async () => {
    const detail = makeBook({ isbn: "978-detail", locked_fields: { language: true } });
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      if (url === "/api/books/b1" && (!init?.method || init.method === "GET")) {
        return { ok: true, status: 200, json: async () => detail };
      }
      return { ok: true, status: 200, json: async () => detail };
    });
    vi.stubGlobal("fetch", fetchMock);

    // A `BookItem` from GET /books: no locked fields (and a possibly stale ISBN).
    const { file_path: _p, file_format: _f, file_parse_status: _s, locked_fields: _l, ...base } = makeBook();
    const listItem: BookItemDto = { ...base, isbn: null, format: "cbz" };
    render(
      <MetadataFieldsProvider catalog={METADATA_FIELDS_CATALOG}>
        <EditBookForm book={listItem} />
      </MetadataFieldsProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: /editBook\.editMetadata/ }));

    await waitFor(() => expect(screen.getByPlaceholderText("ISBN")).toHaveValue("978-detail"));
    fireEvent.submit(screen.getByPlaceholderText("editBook.titlePlaceholder").closest("form")!);

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith("/api/books/b1", expect.objectContaining({ method: "PATCH" })),
    );
    const patch = fetchMock.mock.calls.find(([, init]) => init?.method === "PATCH")!;
    const body = JSON.parse(patch[1]!.body as string);
    expect(body.isbn).toBe("978-detail");
    expect(body.locked_fields).toEqual({ language: true });
  });
  it("reopens with the refreshed detail prop, not the values of the first open", () => {
    stubFetch();
    const { rerender } = render(
      <MetadataFieldsProvider catalog={METADATA_FIELDS_CATALOG}>
        <EditBookForm book={makeBook({ title: "Old title" })} />
      </MetadataFieldsProvider>,
    );
    const trigger = () => screen.getByRole("button", { name: /editBook\.editMetadata/ });
    fireEvent.click(trigger());
    expect(screen.getByPlaceholderText("editBook.titlePlaceholder")).toHaveValue("Old title");
    fireEvent.keyDown(document, { key: "Escape" });

    // e.g. `router.refresh()` after a save, or a metadata refresh elsewhere.
    rerender(
      <MetadataFieldsProvider catalog={METADATA_FIELDS_CATALOG}>
        <EditBookForm book={makeBook({ title: "New title" })} />
      </MetadataFieldsProvider>,
    );
    fireEvent.click(trigger());
    expect(screen.getByPlaceholderText("editBook.titlePlaceholder")).toHaveValue("New title");
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(trigger());
    expect(screen.getByPlaceholderText("editBook.titlePlaceholder")).toHaveValue("New title");
  });

  it("refetches the detail of a list item on every open", async () => {
    let current = makeBook({ isbn: "978-first" });
    const fetchMock = vi.fn(async () => ({ ok: true, status: 200, json: async () => current }));
    vi.stubGlobal("fetch", fetchMock);
    const { file_path: _p, file_format: _f, file_parse_status: _s, locked_fields: _l, ...base } = makeBook();
    const listItem: BookItemDto = { ...base, isbn: null, format: "cbz" };
    render(
      <MetadataFieldsProvider catalog={METADATA_FIELDS_CATALOG}>
        <EditBookForm book={listItem} />
      </MetadataFieldsProvider>,
    );
    const trigger = () => screen.getByRole("button", { name: /editBook\.editMetadata/ });

    fireEvent.click(trigger());
    await waitFor(() => expect(screen.getByPlaceholderText("ISBN")).toHaveValue("978-first"));
    fireEvent.keyDown(document, { key: "Escape" });

    current = makeBook({ isbn: "978-second" });
    fireEvent.click(trigger());
    await waitFor(() => expect(screen.getByPlaceholderText("ISBN")).toHaveValue("978-second"));
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});
