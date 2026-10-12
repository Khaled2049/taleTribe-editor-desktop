// story-data's ceilings (`internal/store`). The desktop app warns at these
// while writing and will refuse to publish past them.
export const CHAPTER_WORD_LIMIT = 5000;
export const STORY_CHAPTER_LIMIT = 50;
export const TITLE_LIMIT = 500;
export const DESCRIPTION_LIMIT = 5000;
export const NAME_LIMIT = 200;
export const PROSE_LIMIT = 20000;
export const MAX_CHAPTER_IMAGES = 5;

// story-data counts whitespace-delimited runs in the stored HTML.
export function chapterWordCount(html: string): number {
  return html.match(/[^ \n\t\r]+/g)?.length ?? 0;
}
