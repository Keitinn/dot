// When the popover's drawing and polling run (design/gdd/companion.md P3).
// No DOM here, so node --test can check it.

/**
 * Only while the popover is shown and focused. The window hides as soon as it
 * loses focus, and the Rust tick keeps the pet going in the meantime.
 */
export function shouldRun({ hidden, focused }) {
    return !hidden && focused;
}
