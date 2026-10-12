import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AiSettings,
  AiStatus,
  AiTask,
  Chapter,
  ChapterSummary,
  Character,
  Generated,
  ImportExisting,
  ImportOutcome,
  ModelInfo,
  Place,
  Provider,
  PublishChoice,
  PublishMode,
  PublishOutcome,
  PublishPlan,
  Snapshot,
  Story,
  WebsiteStory,
} from "./types";

/** The Rust side rejects with `{ kind, message }`; anything else is unexpected. */
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return typeof error === "string" ? error : "Something went wrong.";
}

/** The `code` of an AI failure, such as "notConfigured" or "badKey". */
export function errorCode(error: unknown): string | null {
  if (typeof error === "object" && error !== null && "code" in error) {
    return String((error as { code: unknown }).code);
  }
  return null;
}

export const api = {
  listStories: () => invoke<Story[]>("list_stories"),
  createStory: (title: string) => invoke<Story>("create_story", { title }),
  updateStory: (id: string, title: string, description: string) =>
    invoke<Story>("update_story", { id, title, description }),
  deleteStory: (id: string) => invoke<void>("delete_story", { id }),

  listChapters: (storyId: string) =>
    invoke<ChapterSummary[]>("list_chapters", { storyId }),
  getChapter: (id: string) => invoke<Chapter>("get_chapter", { id }),
  createChapter: (storyId: string, title = "") =>
    invoke<Chapter>("create_chapter", { storyId, title }),
  renameChapter: (id: string, title: string) =>
    invoke<Chapter>("rename_chapter", { id, title }),
  deleteChapter: (id: string) => invoke<void>("delete_chapter", { id }),
  reorderChapters: (storyId: string, orderedIds: string[]) =>
    invoke<ChapterSummary[]>("reorder_chapters", { storyId, orderedIds }),
  saveChapterContent: (id: string, content: string) =>
    invoke<ChapterSummary>("save_chapter_content", { id, content }),

  listSnapshots: (chapterId: string) =>
    invoke<Snapshot[]>("list_snapshots", { chapterId }),
  getSnapshotContent: (id: string) =>
    invoke<string>("get_snapshot_content", { id }),
  restoreSnapshot: (id: string) => invoke<Chapter>("restore_snapshot", { id }),

  listCharacters: (storyId: string) =>
    invoke<Character[]>("list_characters", { storyId }),
  saveCharacter: (character: Character) =>
    invoke<Character>("save_character", { character }),
  deleteCharacter: (id: string) => invoke<void>("delete_character", { id }),

  listPlaces: (storyId: string) => invoke<Place[]>("list_places", { storyId }),
  savePlace: (place: Place) => invoke<Place>("save_place", { place }),
  deletePlace: (id: string) => invoke<void>("delete_place", { id }),

  /** Stores the image locally and returns its asset name. */
  importImage: async (file: Blob) =>
    invoke<string>("import_image", new Uint8Array(await file.arrayBuffer())),

  getAiSettings: () => invoke<AiStatus>("get_ai_settings"),
  saveAiSettings: (settings: AiSettings) =>
    invoke<AiStatus>("save_ai_settings", { settings }),
  setApiKey: (provider: Provider, key: string) =>
    invoke<void>("set_api_key", { provider, key }),
  deleteApiKey: (provider: Provider) =>
    invoke<void>("delete_api_key", { provider }),
  listModels: (provider: Provider, ollamaUrl?: string) =>
    invoke<ModelInfo[]>("list_models", { provider, ollamaUrl }),
  /** `onText` receives each piece of the answer as the model writes it. */
  generate: (requestId: string, task: AiTask, onText: (text: string) => void) => {
    const channel = new Channel<string>();
    channel.onmessage = onText;
    return invoke<Generated>("generate", { requestId, task, onText: channel });
  },
  cancelGeneration: (requestId: string) =>
    invoke<void>("cancel_generation", { requestId }),

  getAccount: () => invoke<Account | null>("get_account"),
  signIn: (email: string, password: string) =>
    invoke<Account>("sign_in", { email, password }),
  signOut: () => invoke<void>("sign_out"),
  publishPreview: (storyId: string) =>
    invoke<PublishPlan>("publish_preview", { storyId }),
  /** `decisions` answers conflicts from an earlier attempt, by conflict id. */
  publish: (
    storyId: string,
    mode: PublishMode,
    decisions: Record<string, PublishChoice>,
    onProgress: (step: string) => void,
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onProgress;
    return invoke<PublishOutcome>("publish", {
      storyId,
      mode,
      decisions,
      onProgress: channel,
    });
  },
  listWebsiteStories: () => invoke<WebsiteStory[]>("list_website_stories"),
  /** `existing` answers an earlier "alreadyHere" outcome. */
  importStory: (
    remoteId: string,
    existing: ImportExisting | null,
    onProgress: (step: string) => void,
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onProgress;
    return invoke<ImportOutcome>("import_story", {
      remoteId,
      existing,
      onProgress: channel,
    });
  },
  /** "story" is the public page; "editor" is where a draft can be seen. */
  openWebsite: (page: "agreement" | "story" | "editor", storyId?: string) =>
    invoke<void>("open_website", { page, storyId }),
};
