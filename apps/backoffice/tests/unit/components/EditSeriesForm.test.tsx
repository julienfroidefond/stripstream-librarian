import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { refresh } = vi.hoisted(() => ({ refresh: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh, push: vi.fn(), replace: vi.fn(), back: vi.fn(), prefetch: vi.fn() }),
}));

import { EditSeriesForm, type EditSeriesFormProps } from "@/app/components/EditSeriesForm";

function makeProps(overrides: Partial<EditSeriesFormProps> = {}): EditSeriesFormProps {
  return {
    libraryId: "l1",
    seriesId: "s1",
    seriesName: "Berserk",
    currentAuthors: ["Kentaro Miura"],
    currentGenres: ["Seinen"],
    currentPublishers: ["Hakusensha"],
    currentBookAuthor: "Kentaro Miura",
    currentBookLanguage: "fr",
    currentDescription: "A dark fantasy",
    currentStartYear: 1989,
    currentTotalVolumes: 41,
    currentStatus: "ongoing",
    currentLockedFields: {},
    ...overrides,
  };
}

function stubFetch(patchResponse: Partial<Response> = {}) {
  const fetchMock = vi.fn((url: string, init?: RequestInit) => {
    if (String(url).includes("/api/series/genres")) {
      return Promise.resolve({ ok: true, json: async () => ["Action", "Drama"] });
    }
    void init;
    return Promise.resolve({ ok: true, json: async () => ({}), ...patchResponse });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

afterEach(() => {
  vi.unstubAllGlobals();
  refresh.mockReset();
});

function openForm(props: EditSeriesFormProps) {
  render(<EditSeriesForm {...props} />);
  fireEvent.click(screen.getByRole("button", { name: /editSeries\.title/ }));
}

describe("EditSeriesForm", () => {
  it("renders the trigger and opens a prefilled modal", () => {
    stubFetch();
    openForm(makeProps());

    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("editSeries.namePlaceholder")).toHaveValue("Berserk");
    expect(screen.getByPlaceholderText("editSeries.startYearPlaceholder")).toHaveValue(1989);
    expect(screen.getByPlaceholderText("12")).toHaveValue(41);
    expect(screen.getByRole("combobox")).toHaveValue("ongoing");
    expect(screen.getByPlaceholderText("editSeries.descriptionPlaceholder")).toHaveValue("A dark fantasy");
  });

  it("leaves the name empty for an unclassified series", () => {
    stubFetch();
    openForm(makeProps({ seriesName: "unclassified" }));

    expect(screen.getByPlaceholderText("editSeries.namePlaceholder")).toHaveValue("");
  });

  it("exposes an open callback to children", () => {
    stubFetch();
    render(
      <EditSeriesForm {...makeProps()}>
        {(open) => (
          <button type="button" onClick={open}>
            custom open
          </button>
        )}
      </EditSeriesForm>
    );

    expect(screen.queryByTestId("modal-panel")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "custom open" }));
    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
  });

  it("adds and removes authors, genres and publishers", () => {
    stubFetch();
    openForm(makeProps({ currentAuthors: [], currentGenres: [], currentPublishers: [] }));

    const author = screen.getByPlaceholderText("editBook.addAuthor");
    fireEvent.change(author, { target: { value: "New Author" } });
    fireEvent.keyDown(author, { key: "Enter" });
    expect(screen.getByText("New Author")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: 'editBook.removeAuthor:{"name":"New Author"}' }));
    expect(screen.queryByText("New Author")).toBeNull();

    const genre = screen.getByPlaceholderText("editSeries.addGenre");
    fireEvent.change(genre, { target: { value: "Action" } });
    fireEvent.keyDown(genre, { key: "Enter" });
    expect(screen.getByText("Action")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: 'editBook.removeAuthor:{"name":"Action"}' }));

    const publisher = screen.getByPlaceholderText("editSeries.addPublisher");
    fireEvent.change(publisher, { target: { value: "Glénat" } });
    fireEvent.keyDown(publisher, { key: "Enter" });
    expect(screen.getByText("Glénat")).toBeInTheDocument();
  });

  it("offers genre suggestions fetched from the API", async () => {
    stubFetch();
    openForm(makeProps({ currentGenres: [] }));

    const genre = screen.getByPlaceholderText("editSeries.addGenre");
    fireEvent.focus(genre);

    await waitFor(() => expect(screen.getByRole("button", { name: "Drama" })).toBeInTheDocument());
    fireEvent.mouseDown(screen.getByRole("button", { name: "Drama" }));

    expect(screen.getByText("Drama")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Action" })).toBeNull();
  });

  it("toggles the apply-to-books panel", () => {
    stubFetch();
    openForm(makeProps());

    expect(screen.queryByPlaceholderText("editSeries.bookAuthorPlaceholder")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "editSeries.applyToBooks" }));
    expect(screen.getByPlaceholderText("editSeries.bookAuthorPlaceholder")).toHaveValue("Kentaro Miura");
    expect(screen.getByPlaceholderText("editBook.languagePlaceholder")).toHaveValue("fr");
  });

  it("toggles the locked-fields legend", () => {
    stubFetch();
    openForm(makeProps());

    expect(screen.queryByText("editBook.lockedFieldsNote")).toBeNull();
    fireEvent.click(screen.getAllByTitle("editBook.clickToLock")[0]);
    expect(screen.getByText("editBook.lockedFieldsNote")).toBeInTheDocument();
  });

  it("disables save when the name is cleared", () => {
    stubFetch();
    openForm(makeProps());

    fireEvent.change(screen.getByPlaceholderText("editSeries.namePlaceholder"), {
      target: { value: "   " },
    });

    expect(screen.getByRole("button", { name: "common.save" })).toBeDisabled();
  });

  it("saves the series, propagating to books when opted in", async () => {
    const fetchMock = stubFetch();
    openForm(makeProps());

    fireEvent.change(screen.getByPlaceholderText("editSeries.namePlaceholder"), {
      target: { value: "  Berserk (FR)  " },
    });
    fireEvent.change(screen.getByPlaceholderText("editSeries.startYearPlaceholder"), {
      target: { value: "1990" },
    });
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "ended" } });
    fireEvent.click(screen.getByRole("button", { name: "editSeries.applyToBooks" }));
    fireEvent.change(screen.getByPlaceholderText("editSeries.bookAuthorPlaceholder"), {
      target: { value: "Miura" },
    });
    fireEvent.submit(screen.getByPlaceholderText("editSeries.namePlaceholder").closest("form")!);

    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/series/s1",
      expect.objectContaining({ method: "PATCH" })
    );
    const patchCall = fetchMock.mock.calls.find(([url]) => url === "/api/series/s1")!;
    const body = JSON.parse((patchCall[1] as RequestInit).body as string);
    expect(body).toMatchObject({
      new_name: "Berserk (FR)",
      start_year: 1990,
      status: "ended",
      author: "Miura",
      language: "fr",
    });
    expect(screen.queryByTestId("modal-panel")).toBeNull();
  });

  it("shows the API error and stays open", async () => {
    stubFetch({ ok: false, json: async () => ({ error: "nope" }) });
    openForm(makeProps());

    fireEvent.submit(screen.getByPlaceholderText("editSeries.namePlaceholder").closest("form")!);

    await waitFor(() => expect(screen.getByText("nope")).toBeInTheDocument());
    expect(screen.getByTestId("modal-panel")).toBeInTheDocument();
    expect(refresh).not.toHaveBeenCalled();
  });

  it("reports a network error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn((url: string) =>
        String(url).includes("/api/series/genres")
          ? Promise.resolve({ ok: true, json: async () => [] })
          : Promise.reject(new Error("offline"))
      )
    );
    openForm(makeProps());

    fireEvent.submit(screen.getByPlaceholderText("editSeries.namePlaceholder").closest("form")!);

    await waitFor(() => expect(screen.getByText("common.networkError")).toBeInTheDocument());
  });
});
