import type { MutableRefObject } from "react";

/**
 * A view with unsaved work registers a guard. The workspace awaits it before
 * navigating away and stays put when it resolves false (the save failed).
 */
export type LeaveGuard = MutableRefObject<(() => Promise<boolean>) | null>;
