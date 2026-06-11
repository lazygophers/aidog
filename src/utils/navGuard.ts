// ─── Navigation guard registry ──────────────────────────────
// A minimal, router-free way to intercept page/tab switches.
//
// The app has no react-router; navigation is plain React state in
// App.tsx (sidebar) and AppSettings.tsx (tab bar). A page that holds
// unsaved changes (e.g. the Claude Code Settings page) registers a
// guard here. Navigators call `requestNavigation(proceed)`:
//   - no guard registered  → `proceed()` runs immediately
//   - guard registered      → the guard decides (e.g. shows a custom
//                             confirm modal) and calls `proceed()` itself
//                             once the user confirms, or drops it on cancel.

type NavGuard = (proceed: () => void) => void;

let activeGuard: NavGuard | null = null;

/**
 * Register a navigation guard. Returns an unregister function.
 * Only one guard is active at a time (last registration wins); the
 * returned cleanup only clears the guard if it is still the active one.
 */
export function registerNavGuard(guard: NavGuard): () => void {
  activeGuard = guard;
  return () => {
    if (activeGuard === guard) activeGuard = null;
  };
}

/**
 * Request a navigation. If a guard is active it mediates the transition;
 * otherwise `proceed` runs synchronously.
 */
export function requestNavigation(proceed: () => void): void {
  if (activeGuard) {
    activeGuard(proceed);
  } else {
    proceed();
  }
}
