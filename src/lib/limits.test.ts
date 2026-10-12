import { describe, expect, it } from "vitest";
import cases from "../../fixtures/word-count.json";
import { chapterWordCount } from "./limits";

// The Rust store reads the same file, so both sides agree with story-data.
describe("chapterWordCount", () => {
  it.each(cases)("counts $words for $html", ({ html, words }) => {
    expect(chapterWordCount(html)).toBe(words);
  });
});
