# TheTaleTribe Desktop

An offline writing app for TheTaleTribe. Stories live on the writer's own
computer; nothing here needs a network or an account. In the TheTaleTribe
workspace this repo is checked out as `repos/taleTribe-desktop`; on GitHub it
is `taleTribe-editor-desktop`.

Built so far: the app shell, local storage, the editor, AI writing help from a
local or bring-your-own-key model, publishing to the website, and importing
from it.

## Commands

```bash
npm install
npm run app:dev      # run the desktop app with hot reload
npm run app:build    # build an installer for this platform

npm run check        # typecheck + lint + TypeScript tests + Rust tests
npm run test         # TypeScript unit tests only
npm run test:rust    # Rust unit tests only
```

Requires Node 20+ and a Rust toolchain (1.82+). Tauri's other system
requirements are at <https://tauri.app/start/prerequisites/>.

## How it is put together

```
src/                  React + TipTap, runs in the Tauri webview
  data/api.ts         the only place that calls into Rust
  editor/schema/      the chapter format (see below)
  editor/             the editor, toolbar and image node view
  hooks/useAutosave   debounced, serialized, retrying save
  features/           library, story workspace, writing help and publish screens
src-tauri/            the native side
  migrations/         SQL, applied in order at startup, never edited once shipped
  src/store.rs        every read and write of story data
  src/assets.rs       image files and the ttasset:// handler
  src/ai/             prompts, story context, providers, settings
  src/sync/           website sign-in, publishing and importing
  src/secrets.rs      the OS keychain (API keys, sign-in token)
  src/commands.rs     the full list of calls the webview may make
fixtures/             test data shared by the TypeScript and Rust tests
```

**The webview never touches SQL or the file system.** It can only call the
commands in `commands.rs`, and it loads images through the `ttasset://` scheme,
which serves files from the app's `assets/` folder and nothing else.

## Where a writer's data is

In the platform's app data folder, under `com.thetaletribe.desktop`:

- macOS: `~/Library/Application Support/com.thetaletribe.desktop/`
- Windows: `%APPDATA%\com.thetaletribe.desktop\`
- Linux: `~/.local/share/com.thetaletribe.desktop/`

`taletribe.db` is the SQLite database and `assets/` holds chapter images. Copying
that folder is a full backup.

## Rules worth knowing before changing things

- **The chapter format is a contract with the website.** Chapters are stored as
  HTML produced by the extensions in `src/editor/schema/`, which are copied
  from the web app's `src/components/editor/`. A change to the web editor's
  TipTap setup needs the same change here, and the reverse.
  `schema.test.ts` fails if opening and saving a chapter would alter it.
- **Word counts follow story-data.** `chapterWordCount` (TypeScript) and
  `word_count` (Rust) both read `fixtures/word-count.json`; add cases there.
- **Limits come from story-data** (`src/lib/limits.ts`, `store.rs`). Sizes that
  can be bounded while typing are; the 5,000-word and 50-chapter limits only
  warn, because publishing is where they are enforced.
- **Never overwrite chapter text without a way back.** `save_chapter_content`
  keeps the text it replaces as a snapshot (at most one per five minutes, the
  newest 20 per chapter), and a restore snapshots the current text first.
- **Local images are referenced as `tt-asset:<name>`** in chapter HTML, the same
  on every platform. Publishing will rewrite these to hosted URLs.

## AI writing help

Optional, and off until the writer picks a provider in AI settings:

- **Ollama** runs a model on the writer's computer; nothing leaves it.
- **Anthropic** and **OpenAI** use the writer's own API key. The text near the
  cursor and the story's notes are sent to that provider. Platform credits and
  creditProxy are not involved.

Rules for this part of the code:

- **Requests are made from Rust, never the webview.** The webview sends only the
  text at the cursor or selection; `src-tauri/src/ai/context.rs` reads the rest
  of the story from the database.
- **API keys live in the OS keychain** (`secrets.rs`). The webview can set or
  remove a key and ask whether one exists, but can never read it back. Keys are
  not in SQLite, and `Secret` prints as `redacted`.
- **Prompts are ported from `taleTribe-agents`** (`agents/storyAgent/tools/`).
  `ai/prompts.rs` names the source file for each; keep the wording in step.
- **Story text in a prompt is data, not instructions.** It goes through
  `sanitize` and inside the `<untrusted_story_data>` wrapper.
- **Model output is inserted as text nodes**, never parsed as HTML
  (`features/ai/textContent.ts`).
- **Tests never call a model.** Request building and stream reading are pure
  functions tested with recorded lines.

On Linux the keychain needs the Secret Service (`libdbus` at build time).

## Publishing to the website

A story is published with the **Publish** button on its screen. The writer signs
in with their website email and password the first time.

- **One direction.** This computer is the source of truth; the website receives
  it. There is no sync and no merging. A repeat publish updates the same
  website story.
- **Nothing is overwritten silently.** A publish first reads the website's copy.
  If a chapter, character, place or the title changed there since this app last
  saw it, the publish stops before writing and the writer chooses, per item, to
  use this computer's version or keep the website's.
- **Interrupted publishes are simply run again.** Every successful write is
  recorded in SQLite straight away (`remote_id`, `remote_revision`, `dirty`,
  `asset_uploads`, `tombstones`).
- **It never touches what it does not know about.** Chapters or characters added
  on the website stay there; fields only the website edits (cover, tags,
  character art and relationships) are read and sent back unchanged.
- **Deleting a local story does not delete the website story.** Deleting a
  published chapter, character or place does remove it at the next publish.
- **Requests are made from Rust**, like AI requests. The sign-in token is in the
  keychain and never reaches the webview, and no backend change is needed: the
  app is a plain story-data client. `sync/remote.rs` is the interface;
  `sync/publish.rs` is tested against an in-memory stand-in for it.
- **story-data's limits are checked before anything is sent** (`sync/plan.rs`),
  and its own refusal messages are shown as written.

### Importing from the website

**Import from website** on the home screen lists the signed-in writer's stories
and copies one onto this computer (`sync/import.rs`).

- **A one-time copy, not a link that stays in sync.** The story remembers which
  website story it is, so publishing later updates that story; nothing is
  pulled again unless the writer imports again.
- **Only the writer's own stories.** story-data serves a published story to any
  signed-in caller, so the importer checks `ownerId` itself.
- **Only the writing comes across:** chapters, characters and places. Comments,
  likes, plots and assistant chats stay on the website. Fields this app does
  not hold (cover, tags, character art and relationships) are not copied either;
  publishing reads them from the website and sends them back unchanged.
- **Images are downloaded** so chapters show offline, and only from the
  website's own storage. One that fails is reported and its address left as is.
- **Importing a story that is already here asks first.** "Replace" makes the
  local story match the website, keeps the replaced text under Versions, and
  never deletes chapters that exist only here. "Keep both" adds a separate
  story that is not connected to the website.
- **Everything is downloaded before anything is written,** in one transaction,
  so a failed import leaves no half-made story.

### Which backend a build talks to

Fixed at build time by environment variables; with none set, a debug build uses
the local stack from `./dev-new.sh` (story-data on 8084, the Firebase emulators,
and story-data's `X-User-ID` dev auth).

| Variable | Meaning |
| --- | --- |
| `TT_STORY_DATA_URL` | story-data's base URL |
| `TT_WEB_URL` | the website, for "view on the website" links |
| `TT_FIREBASE_API_KEY` | the Firebase web API key (required for a real build) |
| `TT_FIREBASE_PROJECT_ID` | Firebase project, used for the Cloud Functions URL |
| `TT_STORAGE_BUCKET` | Firebase Storage bucket for chapter images |
| `TT_USE_EMULATORS` | `true` or `false`; defaults to `true` in debug builds only |

A release build without `TT_FIREBASE_API_KEY` refuses to sign in rather than
guess.

## Not here yet

Installers, code signing and auto-update are not built yet. The Story Assistant chat is not planned for this app yet.
