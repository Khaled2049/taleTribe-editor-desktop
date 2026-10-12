# Repository guidelines: taleTribe-desktop

The offline desktop editor for TheTaleTribe: a Tauri 2 app with a Rust native
side (`src-tauri/`) and a React + TipTap webview (`src/`). `README.md` explains
what the app does and the reasoning behind each rule below; read it first.

## Commands

```bash
npm run app:dev      # run the app with hot reload
npm run check        # typecheck + lint + TypeScript tests + Rust tests
npm run test         # TypeScript unit tests (vitest, jsdom)
npm run test:rust    # Rust unit tests
(cd src-tauri && cargo fmt && cargo clippy --all-targets -- -D warnings)
```

Run `npm run check` and the `cargo fmt` + `clippy` line before calling a change
done; `check` does not run them. All tests are unit tests and need no network,
no local stack and no model.

## Architecture rules

- **The webview only calls commands.** It never touches SQL, files or the
  network. `src/data/api.ts` is the one place that calls `invoke`; everything
  it can reach is listed in `src-tauri/src/commands.rs`. The CSP in
  `tauri.conf.json` allows no outbound connections, and should stay that way.
- **Adding a command takes three edits:** the function in `commands.rs`, its
  name in the `generate_handler!` list in `lib.rs`, and a typed wrapper in
  `src/data/api.ts`. Keep commands thin; logic belongs in `store.rs`, `ai/` or
  `sync/`.
- **Secrets stay native.** API keys and the website sign-in token live in the
  OS keychain (`secrets.rs`) and are never returned to the webview, written to
  SQLite, or logged. Wrap one in `Secret` so it cannot be printed.
- **Never hold the database lock across an `await`.** `AppState.db` is a
  `std::sync::Mutex`. Read what you need, drop the guard, then make the
  request. `Publisher::db` and `Importer::db` show the pattern.
- **Migrations are append-only.** Add a new numbered file in
  `src-tauri/migrations/` and list it in `db.rs`. Never edit one that exists:
  writers' databases have already applied it.
- **Chapter text is never overwritten without a way back.** Go through
  `save_chapter_content` or `overwrite_chapter_content`, which keep a snapshot.

## Contracts with the other repos

These are re-implemented or copied here. A change on either side needs the
matching change on the other.

- **Chapter HTML format:** `src/editor/schema/` is a copy of the web editor's
  TipTap extensions (`taleTribe-frontend/src/components/editor/`). The list in
  `schema/index.ts` must match the web's `TipTapEditor.tsx`, in order and
  options. `schema.test.ts` and `fixtures/chapters/` guard the round trip. One
  deliberate difference: headings carry no per-level class (see the comment in
  `schema/index.ts`).
- **story-data limits and word count:** `src/lib/limits.ts`, `store.rs` and
  `sync/plan.rs`. Word-count cases live in `fixtures/word-count.json`, which
  both the TypeScript and Rust tests read.
- **story-data's API:** `sync/remote.rs` is the interface, `sync/http.rs` the
  HTTP implementation. An update there replaces the whole record, so fields
  this app does not edit are read from the website and sent back unchanged.
- **Generation prompts:** `ai/prompts.rs` is ported from
  `taleTribe-agents/agents/storyAgent/tools/`.

## Testing

- Prefer a few tests of behaviour that would lose a writer's work over broad
  coverage. Publishing and importing are tested against the in-memory website
  in `sync/fake.rs`; add cases there, not network tests.
- Tests never call a model or a server. Provider request building and stream
  parsing are pure functions tested with recorded lines.
- In vitest, write `beforeEach(() => { mock.mockReset(); })` with braces. A
  function returned from `beforeEach` is run as a cleanup hook, and
  `mockReset()` returns the mock.
- jsdom has no layout: dialog tests stub `showModal`, and editor tests stub the
  `Range` rectangle methods (see `App.test.tsx`).

## Working on this locally

- A debug build with no `TT_*` variables talks to the local stack from the
  workspace's `./dev-new.sh` (story-data on 8084 and the Firebase emulators)
  and signs in as the seeded dev user. Writing and Ollama need no stack at all.
- `npm run app:dev` uses the real app data folder
  (`~/Library/Application Support/com.thetaletribe.desktop/` on macOS), so
  stories created while developing persist, and a new migration runs against
  that database.
- Any Rust change makes `tauri dev` rebuild and restart the app. On macOS a
  rebuilt debug binary is a new program to the keychain, so it asks for
  permission again; "Always Allow" lasts until the next rebuild.
- Publishing from a debug build writes to the local story-data and the Storage
  emulator. Clean up stories created while testing.
