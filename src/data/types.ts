export interface Story {
  id: string;
  title: string;
  description: string;
  chapterCount: number;
  wordCount: number;
  createdAt: number;
  updatedAt: number;
}

export interface ChapterSummary {
  id: string;
  storyId: string;
  title: string;
  wordCount: number;
  position: number;
  updatedAt: number;
}

export interface Chapter extends ChapterSummary {
  content: string;
}

export interface Snapshot {
  id: string;
  chapterId: string;
  wordCount: number;
  createdAt: number;
}

export interface Character {
  /** Empty for a character that has not been saved yet. */
  id: string;
  storyId: string;
  name: string;
  age: number | null;
  soul: string;
  personality: string;
  voice: string;
  backstory: string;
  affiliations: string;
  notes: string;
}

export interface Place {
  /** Empty for a place that has not been saved yet. */
  id: string;
  storyId: string;
  name: string;
  description: string;
  atmosphere: string;
  geography: string;
  history: string;
  significance: string;
  notes: string;
}

export type Provider = "ollama" | "anthropic" | "openai";

export interface ModelChoice {
  id: string;
  supportsEffort: boolean;
  canThink: boolean;
}

export interface ModelInfo extends ModelChoice {
  label: string;
}

export interface AiSettings {
  /** Null until the writer sets AI up. */
  provider: Provider | null;
  ollamaUrl: string;
  ollama: ModelChoice;
  anthropic: ModelChoice;
  openai: ModelChoice;
  contextBudget: "small" | "medium" | "large";
}

/** Settings plus whether a key is saved. The key itself never reaches the webview. */
export interface AiStatus {
  settings: AiSettings;
  hasAnthropicKey: boolean;
  hasOpenaiKey: boolean;
}

export type EnhanceAction = "expand" | "dialogue" | "rewrite";
export type IdeaKind = "characters" | "plots" | "places" | "themes";

export type AiTask =
  | {
      task: "nextLines";
      storyId: string;
      chapterId: string;
      before: string;
      after: string;
    }
  | { task: "enhance"; storyId: string; action: EnhanceAction; text: string }
  | { task: "brainstorm"; storyId: string; kind: IdeaKind; prompt: string };

export interface Generated {
  text: string;
  /** Present for next-line requests. */
  suggestions: string[] | null;
}

export interface Account {
  uid: string;
  email: string;
}

export interface PublishCounts {
  new: number;
  changed: number;
  deleted: number;
}

/** What a publish would send, worked out locally. */
export interface PublishPlan {
  /** Already on the website; publishing updates it. */
  linked: boolean;
  storyChanged: boolean;
  chapters: PublishCounts;
  characters: PublishCounts;
  places: PublishCounts;
  imagesToUpload: number;
  /** Reasons the website would refuse this story. */
  problems: string[];
  nothingToSend: boolean;
}

export type PublishMode = "draft" | "publish";
export type PublishChoice = "overwrite" | "skip";

/** Something changed on the website since this app last saw it. */
export interface PublishConflict {
  id: string;
  kind: "story" | "chapter" | "character" | "place";
  label: string;
  /** Deleted on this computer; overwriting deletes it on the website too. */
  deletedHere: boolean;
}

export interface PublishSummary {
  created: number;
  updated: number;
  deleted: number;
  images: number;
  released: number;
}

export type PublishOutcome =
  | { status: "done"; remoteId: string; public: boolean; summary: PublishSummary }
  | { status: "needsAgreement" }
  | { status: "blocked"; problems: string[] }
  | { status: "conflicts"; conflicts: PublishConflict[] }
  | { status: "gone" };

/** One of the writer's stories on the website. */
export interface WebsiteStory {
  id: string;
  title: string;
  description: string;
  published: boolean;
  chapterCount: number;
  wordCount: number;
  updatedAt: string;
  /** The local story already holding it, if there is one. */
  localId: string | null;
}

/** What to do when a story being imported is already on this computer. */
export type ImportExisting = "replace" | "copy";

export type ImportOutcome =
  | {
      status: "done";
      storyId: string;
      chapters: number;
      characters: number;
      places: number;
      images: number;
      /** Images that could not be downloaded. */
      imagesMissing: number;
      /** Chapters that exist only on this computer and were kept. */
      keptLocal: number;
      /** False for a separate copy, which publishes as a new story. */
      linked: boolean;
    }
  | { status: "alreadyHere"; storyId: string; title: string };
