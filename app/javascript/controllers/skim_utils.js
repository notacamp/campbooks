// Shared Skim viewer helpers — the fussiest bits (the oklch segment/tint math and
// the toast toggle) factored out so the document Skim controller can reuse them
// without coupling to the live email skim_mode_controller. Pure functions: they
// take the elements they touch, hold no state.

export function isDark() {
  return document.documentElement.classList.contains("dark")
}

// Build the segmented "stories" progress bar into `container`: up to MAX_SEGMENTS
// discrete segments, degrading to a single proportional bar beyond that.
export function buildSegments(container, total, pos, hue) {
  if (!container) return
  container.replaceChildren()
  const dark = isDark()
  const track = dark ? "oklch(30% 0.01 60)" : "oklch(91% 0.004 60)"
  const current = dark ? "oklch(97% 0.003 60)" : "oklch(20% 0.006 60)"
  const filled = current
  const MAX_SEGMENTS = 16

  if (total > MAX_SEGMENTS) {
    const bar = document.createElement("div")
    bar.style.cssText = `flex:1 1 0;height:3px;border-radius:9999px;overflow:hidden;background:${track}`
    const fill = document.createElement("div")
    fill.style.cssText = `height:100%;border-radius:9999px;transition:width .25s cubic-bezier(0.16,1,0.3,1);width:${Math.round((pos / total) * 100)}%;background:${current}`
    bar.appendChild(fill)
    container.appendChild(bar)
    return
  }

  for (let i = 1; i <= total; i++) {
    const seg = document.createElement("div")
    const color = i < pos ? filled : i === pos ? current : track
    seg.style.cssText = `flex:1 1 0;height:3px;border-radius:9999px;transition:background-color .25s cubic-bezier(0.16,1,0.3,1);background:${color}`
    container.appendChild(seg)
  }
}

// True while any full-screen Skim overlay (inbox or document) is open. The
// overlays are plain role="dialog" panels with a toggled `hidden` class — NOT
// native <dialog open> — so the window/document-level keyboard handlers can't
// detect them the usual way and would otherwise also fire their shortcuts on the
// inbox/feed behind the overlay. Those handlers consult this to stay silent while
// Skim has the keyboard.
export function skimOverlayOpen() {
  return !!document.querySelector(
    '[data-skim-overlay-target="panel"]:not(.hidden), [data-doc-skim-overlay-target="panel"]:not(.hidden)'
  )
}

// Tint a full-screen viewer element's background to the current ring hue.
export function tint(element, hue) {
  element.style.background = isDark()
    ? "oklch(16% 0.005 60)"
    : "oklch(98.5% 0.002 60)"
}

// Reveal a hidden StatusFeedback toast, set its message, toggle its Undo button,
// and schedule auto-dismiss. Returns the timer id so the caller can cancel it.
export function showToast({ toast, messageEl, icon, undo, text, undoable, success = true, durationMs, onHide }) {
  if (!toast) return null
  if (messageEl) messageEl.textContent = text
  if (icon) icon.classList.toggle("hidden", !success)
  if (undo) undo.classList.toggle("hidden", !undoable)
  toast.classList.remove("hidden")
  toast.classList.add("flex")
  return setTimeout(() => {
    toast.classList.add("hidden")
    toast.classList.remove("flex")
    if (onHide) onHide()
  }, durationMs)
}
