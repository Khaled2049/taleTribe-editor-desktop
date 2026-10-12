import { useState } from "react";
import { Library } from "./features/library/Library";
import { StoryWorkspace } from "./features/story/StoryWorkspace";

export function App() {
  const [storyId, setStoryId] = useState<string | null>(null);
  return storyId ? (
    <StoryWorkspace
      key={storyId}
      storyId={storyId}
      onBack={() => setStoryId(null)}
    />
  ) : (
    <Library onOpen={setStoryId} />
  );
}
