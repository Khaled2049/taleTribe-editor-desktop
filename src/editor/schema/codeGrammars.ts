import type { createLowlight } from "lowlight";
import javascript from "highlight.js/lib/languages/javascript";
import typescript from "highlight.js/lib/languages/typescript";
import python from "highlight.js/lib/languages/python";
import css from "highlight.js/lib/languages/css";
import xml from "highlight.js/lib/languages/xml";
import bash from "highlight.js/lib/languages/bash";
import json from "highlight.js/lib/languages/json";
import rust from "highlight.js/lib/languages/rust";
import go from "highlight.js/lib/languages/go";

export function registerCodeGrammars(
  lowlight: ReturnType<typeof createLowlight>,
) {
  lowlight.register("javascript", javascript);
  lowlight.register("js", javascript);
  lowlight.register("typescript", typescript);
  lowlight.register("ts", typescript);
  lowlight.register("python", python);
  lowlight.register("py", python);
  lowlight.register("css", css);
  lowlight.register("html", xml);
  lowlight.register("xml", xml);
  lowlight.register("bash", bash);
  lowlight.register("sh", bash);
  lowlight.register("json", json);
  lowlight.register("rust", rust);
  lowlight.register("go", go);
}
