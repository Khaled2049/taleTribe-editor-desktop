import { convertFileSrc } from "@tauri-apps/api/core";

/**
 * Chapter HTML refers to a local image as `tt-asset:<name>`. That form is the
 * same on every platform and is what publish will rewrite to a hosted URL.
 */
const ASSET_PREFIX = "tt-asset:";

export const assetSrc = (name: string) => `${ASSET_PREFIX}${name}`;

/** The URL the webview can load; differs between macOS/Linux and Windows. */
export function displaySrc(src: string): string {
  if (!src.startsWith(ASSET_PREFIX)) return src;
  return convertFileSrc(src.slice(ASSET_PREFIX.length), "ttasset");
}
