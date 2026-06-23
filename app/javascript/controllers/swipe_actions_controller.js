import { Controller } from "@hotwired/stimulus"

// One global hook (registered once on first controller connect): when ANY Turbo
// Stream removes a swipeable row — keyboard shortcut, Scout, bulk action, the
// reply-daemon, the inline Dismiss button — play the same collapse + fade as a
// swipe, so rows never just blink out. Swipe-initiated removals set
// data-swipe-removing and animate themselves (directionally), so the hook leaves
// those alone — no double animation.
let removeHookRegistered = false
function registerRemoveAnimationHook() {
  if (removeHookRegistered) return
  removeHookRegistered = true
  document.addEventListener("turbo:before-stream-render", (event) => {
    const stream = event.target
    if (!stream || stream.getAttribute("action") !== "remove") return
    const target = document.getElementById(stream.getAttribute("target") || "")
    if (!target || target.dataset.swipeRemoving === "true") return
    if (!target.matches('[data-controller~="swipe-actions"]')) return
    const render = event.detail.render
    event.detail.render = (streamElement) => collapseAndFadeOut(target).then(() => render(streamElement))
  })
}

function collapseAndFadeOut(el) {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return Promise.resolve()
  return new Promise((resolve) => {
    const height = el.getBoundingClientRect().height
    el.style.height = `${height}px`
    el.style.overflow = "hidden"
    el.getBoundingClientRect() // reflow so the height transition has a start value
    el.style.transition = "height 260ms cubic-bezier(0.22, 1, 0.36, 1), opacity 200ms ease-out, transform 260ms cubic-bezier(0.22, 1, 0.36, 1)"
    el.style.height = "0"
    el.style.opacity = "0"
    el.style.transform = "translateX(-16px)"
    let settled = false
    const finish = () => { if (settled) return; settled = true; el.removeEventListener("transitionend", finish); resolve() }
    el.addEventListener("transitionend", finish)
    setTimeout(finish, 340) // fallback if transitionend doesn't fire
  })
}

// Generic swipe / drag-to-act for any list row, rendered by Campbooks::Swipeable.
//
// One instance per row. The row content (the yielded markup) sits in the
// `content` target and slides horizontally; behind it, a colored action panel
// (icon + label) is revealed and intensifies as you drag, then MORPHS color +
// icon when you cross a second threshold (progressive two-stage). Releasing past
// the first threshold commits the armed action; releasing before it springs back.
//
// Direction is named from the user's point of view (matches Campbooks::Swipeable):
//   `leftValue`  = stages fired by swiping the content LEFT  (reveals the RIGHT-anchored panel)
//   `rightValue` = stages fired by swiping the content RIGHT (reveals the LEFT-anchored panel)
//
// Each stage is { key, label, iconSvg, color, endpoint, method, params,
// confirm, picker, removes }. A `confirm` gate shows a shared dialog; a `picker`
// (e.g. "snooze") collects an argument first; `removes` (default true) means a
// committed action removes the row (optimistic fly-out + height-collapse, then
// the server Turbo Stream finalizes), vs. replace-in-place (docs approve/reject).
//
// Mirrors house style: pointer events + setPointerCapture (calendar_dnd), a
// movement threshold + dy/dx ratio guard so vertical scroll is never hijacked
// (skim_mode), and fetch -> Turbo.renderStreamMessage (email_shortcuts).
export default class extends Controller {
  static targets = [
    "content",
    "leftPanel", "leftIcon", "leftLabel",
    "rightPanel", "rightIcon", "rightLabel"
  ]
  static values = {
    left: { type: Array, default: [] },
    right: { type: Array, default: [] },
    errorMessage: { type: String, default: "Something went wrong" }
  }

  // Fraction of the row's width at which each stage arms.
  static T1 = 0.25
  static T2 = 0.5

  connect() {
    registerRemoveAnimationHook()
    // Let the browser keep vertical scrolling; we only own horizontal drags.
    this.element.style.touchAction = "pan-y"
    this._onDown = this._down.bind(this)
    this._onClick = this._click.bind(this)
    // Rows usually wrap an <a>/<img>; without this the browser starts a native
    // link/image drag the moment you move and swallows our pointer events. The
    // exception is an explicit [data-drag-handle] (drag-to-folder): let that drag
    // run and stash the row's email ids for the folder drop target.
    this._onDragStart = (e) => this._dragStart(e)
    this.element.addEventListener("pointerdown", this._onDown)
    this.element.addEventListener("dragstart", this._onDragStart)
    // Capture-phase: cancel the click that follows a real drag so a swipe never
    // also opens the row's link. A genuine tap leaves _suppressClick false.
    this.element.addEventListener("click", this._onClick, { capture: true })
  }

  disconnect() {
    this.element.removeEventListener("pointerdown", this._onDown)
    this.element.removeEventListener("dragstart", this._onDragStart)
    this.element.removeEventListener("click", this._onClick, { capture: true })
    this._teardown()
  }

  // --- Programmatic trigger (keyboard) ---

  // Fire the primary stage of a direction on this row, reusing the full commit
  // pipeline (confirm gate + fly-out animation), so the inbox ArrowLeft/ArrowRight
  // shortcuts do exactly what a swipe does.
  triggerStage(direction) {
    if (this._committing) return
    const stages = this._stagesFor(direction)
    if (!stages || stages.length === 0) return
    this._direction = direction
    this._commit(stages[0])
  }

  // --- Pointer flow ---

  _down(e) {
    if (e.button !== 0) return // primary button / touch only
    // Don't start a drag from an interactive control (checkbox, action button…);
    // the row link is fine — the click-suppressor sorts tap vs. drag.
    if (e.target.closest("input, button, select, textarea, label, [contenteditable], [data-no-swipe]")) return
    if (this._committing) return

    this._startX = e.clientX
    this._startY = e.clientY
    this._width = this.element.getBoundingClientRect().width
    this._state = "pending" // pending -> locked | cancelled
    this._direction = null
    this._stage = 0
    this._dx = 0
    this._moved = false
    this._suppressClick = false
    this._pointerId = e.pointerId

    this._onMove = this._move.bind(this)
    this._onUp = this._up.bind(this)
    this._onCancel = this._cancel.bind(this)
    this.element.addEventListener("pointermove", this._onMove)
    this.element.addEventListener("pointerup", this._onUp)
    this.element.addEventListener("pointercancel", this._onCancel)
  }

  _move(e) {
    if (this._state === "cancelled") return
    const dx = e.clientX - this._startX
    const dy = e.clientY - this._startY

    if (this._state === "pending") {
      const adx = Math.abs(dx)
      const ady = Math.abs(dy)
      if (adx < 6 && ady < 6) return // below the slop radius — undecided
      if (ady > adx * 1.5) { this._cancelDrag(); return } // vertical intent => let it scroll

      const direction = dx > 0 ? "right" : "left"
      if (this._stagesFor(direction).length === 0) { this._cancelDrag(); return } // inert side

      this._state = "locked"
      this._direction = direction
      try { this.element.setPointerCapture(this._pointerId) } catch (_e) { /* pointer already gone */ }
      this.element.dataset.swiping = "true" // CSS gives content an opaque surface
    }

    e.preventDefault()
    this._moved = true

    const stages = this._stagesFor(this._direction)
    const t1 = this._width * this.constructor.T1
    const t2 = this._width * this.constructor.T2
    const adx = Math.abs(dx)
    this._dx = adx

    const max = t2 + 24 // a little overscroll past the deep threshold
    const magnitude = Math.min(adx, max)
    const translate = this._direction === "left" ? -magnitude : magnitude

    const stage = adx >= t2 && stages.length > 1 ? 2 : adx >= t1 ? 1 : 0
    this._render(translate, stage, stages, t1)
  }

  _up() {
    const committed = this._state === "locked" && this._moved
    const stages = committed ? this._stagesFor(this._direction) : []
    const t1 = this._width * this.constructor.T1

    this._teardown()

    if (!committed) return
    if (this._dx < t1) { this._snapBack(); return }

    this._suppressClick = true // a real drag happened; swallow the trailing click
    const cfg = this._stage === 2 && stages.length > 1 ? stages[1] : stages[0]
    this._commit(cfg)
  }

  _cancel() {
    const wasLocked = this._state === "locked" && this._moved
    this._teardown()
    if (wasLocked) this._snapBack()
  }

  // Abort an undecided/vertical gesture without capturing the pointer.
  _cancelDrag() {
    this._state = "cancelled"
    this._removeMoveListeners()
  }

  _click(e) {
    if (this._suppressClick) {
      e.preventDefault()
      e.stopPropagation()
      this._suppressClick = false
    }
  }

  // Native HTML5 drag: blocked everywhere except the explicit folder drag handle,
  // whose payload (the thread's email ids) the mail-folder-drop controller reads.
  _dragStart(e) {
    const handle = e.target.closest("[data-drag-handle]")
    if (!handle) { e.preventDefault(); return }
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move"
      e.dataTransfer.setData("text/plain", handle.dataset.emailIds || "")
    }
  }

  // --- Visuals ---

  _render(translate, stage, stages, t1) {
    this.contentTarget.style.transition = "none"
    this.contentTarget.style.transform = `translateX(${translate}px)`

    const { panel, icon, label } = this._side(this._direction)
    const other = this._side(this._direction === "left" ? "right" : "left").panel
    if (other) { other.style.display = "none"; other.style.opacity = "0" }
    if (!panel) return

    panel.style.display = "flex"
    panel.style.opacity = String(Math.min(Math.abs(translate) / t1, 1))

    if (stage !== this._stage) {
      this._stage = stage
      const cfg = stage === 2 && stages.length > 1 ? stages[1] : stages[0]
      this._paint(panel, icon, label, cfg)
      if (stage > 0 && !this._reduced()) this._pulse(panel)
    }
  }

  _paint(panel, icon, label, cfg) {
    panel.dataset.color = cfg.color
    if (icon) icon.innerHTML = cfg.iconSvg || ""
    if (label) label.textContent = cfg.label || ""
  }

  _pulse(panel) {
    panel.style.transition = "transform 130ms cubic-bezier(0.22, 1, 0.36, 1)"
    panel.style.transform = "scale(1.06)"
    setTimeout(() => { panel.style.transform = ""; panel.style.transition = "" }, 130)
  }

  _snapBack() {
    this.element.dataset.swiping = "false"
    if (this._reduced()) {
      this.contentTarget.style.transform = ""
      this._resetPanels()
      return
    }
    // Exponential ease-out, no bounce (DESIGN.md motion law).
    this.contentTarget.style.transition = "transform 320ms cubic-bezier(0.22, 1, 0.36, 1)"
    this.contentTarget.style.transform = "translateX(0)"
    const done = () => {
      this.contentTarget.style.transition = ""
      this.contentTarget.removeEventListener("transitionend", done)
      this._resetPanels()
    }
    this.contentTarget.addEventListener("transitionend", done)
  }

  _resetPanels() {
    delete this.element.dataset.swiping
    for (const p of [this.hasLeftPanelTarget && this.leftPanelTarget, this.hasRightPanelTarget && this.rightPanelTarget]) {
      if (!p) continue
      p.style.display = "none"
      p.style.opacity = "0"
      p.style.transform = ""
    }
    this._stage = 0
  }

  // --- Commit ---

  async _commit(cfg) {
    this._committing = true
    try {
      if (cfg.confirm && !this._confirmRemembered(cfg.confirm)) {
        const { confirmed, remember } = await this._askConfirm({ ...cfg.confirm, color: cfg.color })
        if (!confirmed) { this._committing = false; this._snapBack(); return }
        if (remember) this._rememberConfirm(cfg.confirm)
      }
      if (cfg.picker === "snooze") {
        const snoozedUntil = await this._ask("snooze", {})
        if (!snoozedUntil) { this._committing = false; this._snapBack(); return }
        cfg = { ...cfg, params: { ...cfg.params, "args[snoozed_until]": snoozedUntil } }
      }

      const removes = cfg.removes !== false
      // Remove-style: fly the row out + collapse so siblings slide up, then the
      // server stream finalizes. Replace-style (docs): spring home now; the stream
      // swaps the row in place when it returns.
      if (removes) this._slideOut()
      else this._snapBack()

      const html = await this._post(cfg)
      if (window.Turbo && html) window.Turbo.renderStreamMessage(html)

      if (removes) {
        // The server stream removed the inner row (and advanced the reader, if it
        // was open). Clean up our wrapper shell after the collapse plays. Safe even
        // when the wrapper itself was the stream's target (already detached).
        requestAnimationFrame(() => this.element.remove())
      } else {
        this._committing = false
      }
    } catch (_e) {
      this._committing = false
      this._restoreAfterError()
      this._toastError()
    }
  }

  _slideOut() {
    this.element.dataset.swipeRemoving = "true" // we animate this one; the global remove-hook skips it
    if (this._reduced()) { this.element.style.display = "none"; return }
    const off = this._direction === "left" ? "-110%" : "110%"
    this.contentTarget.style.transition = "transform 240ms cubic-bezier(0.16, 1, 0.3, 1)"
    this.contentTarget.style.transform = `translateX(${off})`

    this._collapsedFrom = this.element.getBoundingClientRect().height
    this.element.style.height = `${this._collapsedFrom}px`
    this.element.style.overflow = "hidden"
    requestAnimationFrame(() => {
      this.element.style.transition = "height 220ms ease-in 120ms, opacity 220ms ease-in"
      this.element.style.height = "0"
      this.element.style.opacity = "0"
    })
  }

  _restoreAfterError() {
    delete this.element.dataset.swipeRemoving
    this.element.style.transition = ""
    this.element.style.height = ""
    this.element.style.overflow = ""
    this.element.style.opacity = ""
    this.element.style.display = ""
    this._snapBack()
  }

  _post(cfg) {
    const body = new URLSearchParams()
    for (const [k, v] of Object.entries(cfg.params || {})) {
      if (Array.isArray(v)) v.forEach((x) => body.append(k, x))
      else body.append(k, v)
    }
    return fetch(cfg.endpoint, {
      method: (cfg.method || "post").toUpperCase(),
      headers: {
        "X-CSRF-Token": this._csrf(),
        "Accept": "text/vnd.turbo-stream.html",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: body.toString()
    }).then((r) => (r.ok ? r.text() : Promise.reject(r)))
  }

  // --- Shared dialog round-trips (confirm / snooze picker) ---

  // Resolves with the user's choice: confirm -> bool, snooze -> ISO string|null.
  _ask(kind, detail) {
    return new Promise((resolve) => {
      const id = `swipe-${kind}-${this._pointerId}-${performance.now()}`
      const onResponse = (e) => {
        if (e.detail.id !== id) return
        window.removeEventListener(`swipe-actions:${kind}-response`, onResponse)
        resolve(kind === "confirm" ? !!e.detail.confirmed : e.detail.value || null)
      }
      window.addEventListener(`swipe-actions:${kind}-response`, onResponse)
      window.dispatchEvent(new CustomEvent(`swipe-actions:${kind}-request`, { detail: { id, ...detail } }))
    })
  }

  // Confirm round-trip that also reports the user's "don't ask again" choice.
  _askConfirm(detail) {
    return new Promise((resolve) => {
      const id = `swipe-confirm-${this._pointerId}-${performance.now()}`
      const onResponse = (e) => {
        if (e.detail.id !== id) return
        window.removeEventListener("swipe-actions:confirm-response", onResponse)
        resolve({ confirmed: !!e.detail.confirmed, remember: !!e.detail.dontAskAgain })
      }
      window.addEventListener("swipe-actions:confirm-response", onResponse)
      window.dispatchEvent(new CustomEvent("swipe-actions:confirm-request", { detail: { id, ...detail } }))
    })
  }

  _confirmStorageKey(confirm) {
    return confirm.rememberKey ? `campbooks:skip_confirm:${confirm.rememberKey}` : null
  }

  _confirmRemembered(confirm) {
    const key = this._confirmStorageKey(confirm)
    if (!key) return false
    try { return localStorage.getItem(key) === "true" } catch (_e) { return false }
  }

  _rememberConfirm(confirm) {
    const key = this._confirmStorageKey(confirm)
    if (!key) return
    try { localStorage.setItem(key, "true") } catch (_e) { /* private mode: just ask again next time */ }
  }

  // Client-built error snackbar (a swipe can fail with no server round-trip).
  // Mirrors Campbooks::ActionToast's error markup so it matches server toasts.
  _toastError() {
    if (!window.Turbo) return
    const msg = this.errorMessageValue
    window.Turbo.renderStreamMessage(
      `<turbo-stream action="append" target="action_toasts"><template>` +
      `<div class="pointer-events-auto inline-flex max-w-full items-center gap-2.5 rounded-full border border-border bg-card/95 py-1.5 pl-2.5 pr-3 text-sm font-medium text-foreground shadow-lg backdrop-blur animate-fade-in" role="status" aria-live="polite" data-action-toast-duration="4000">` +
      `<span class="inline-flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full bg-red-100 text-red-700 dark:bg-red-500/15 dark:text-red-300">` +
      `<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg></span>` +
      `<span class="min-w-0">${msg}</span></div></template></turbo-stream>`
    )
  }

  // --- Helpers ---

  _stagesFor(direction) {
    return direction === "left" ? this.leftValue : this.rightValue
  }

  // Map a swipe direction to the panel it reveals + that panel's icon/label slots.
  // Swiping left reveals the right-anchored panel and vice-versa.
  _side(direction) {
    if (direction === "left") {
      return {
        panel: this.hasRightPanelTarget ? this.rightPanelTarget : null,
        icon: this.hasRightIconTarget ? this.rightIconTarget : null,
        label: this.hasRightLabelTarget ? this.rightLabelTarget : null
      }
    }
    return {
      panel: this.hasLeftPanelTarget ? this.leftPanelTarget : null,
      icon: this.hasLeftIconTarget ? this.leftIconTarget : null,
      label: this.hasLeftLabelTarget ? this.leftLabelTarget : null
    }
  }

  _teardown() {
    this._removeMoveListeners()
    if (this._pointerId != null && this.element.hasPointerCapture?.(this._pointerId)) {
      this.element.releasePointerCapture(this._pointerId)
    }
    this._state = "idle"
  }

  _removeMoveListeners() {
    if (this._onMove) this.element.removeEventListener("pointermove", this._onMove)
    if (this._onUp) this.element.removeEventListener("pointerup", this._onUp)
    if (this._onCancel) this.element.removeEventListener("pointercancel", this._onCancel)
  }

  _reduced() {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches
  }

  _csrf() {
    return document.querySelector('meta[name="csrf-token"]')?.content
  }
}
