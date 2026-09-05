// hint_controller.js — Global shortcut-hint tooltip.
//
// Contract:
//   data-hint="<Label>"          — the action's name (required)
//   data-hint-key="<key>"        — e.g. "r", "g p", "⌘K" (optional)
//   data-hint-placement="<dir>"  — "top" (default) or "right" (rails)
//   aria-keyshortcuts="<ks>"     — WAI-ARIA keyshortcuts attribute on the same element
//
// The controller is mounted on <body> and uses delegated listeners. One
// tooltip element (Campbooks::HintTip, rendered once per layout) is shared
// across the whole page. Only fine-pointer devices (not touch) trigger hints.
//
// Key notation: single ASCII letter → upper-cased cap; anything else verbatim
// (⌘K, ⇧I, Esc, ⏎, →, ←, g p = two separate caps for a chord sequence).

import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["tip", "label", "keys"]

  connect() {
    this.fine = window.matchMedia("(hover: hover) and (pointer: fine)").matches
    if (!this.fine || !this.hasTipTarget) return

    this._onOver       = this._onOver.bind(this)
    this._onOut        = this._onOut.bind(this)
    this._onPress      = this._onPress.bind(this)
    this._cancelAndHide = this._cancelAndHide.bind(this)
    this._onFocusIn    = this._onFocusIn.bind(this)
    this._onFocusOut   = this._onFocusOut.bind(this)

    document.addEventListener("pointerover",  this._onOver)
    document.addEventListener("pointerout",   this._onOut)
    document.addEventListener("pointerdown",  this._onPress,       { capture: true })
    document.addEventListener("keydown",      this._cancelAndHide, { capture: true })
    document.addEventListener("scroll",       this._cancelAndHide, { capture: true, passive: true })
    document.addEventListener("focusin",      this._onFocusIn)
    document.addEventListener("focusout",     this._onFocusOut)

    const turboEvents = [
      "turbo:before-visit",
      "turbo:before-render",
      "turbo:render",
      "turbo:frame-render",
      "turbo:before-stream-render",
    ]
    turboEvents.forEach(ev => document.addEventListener(ev, this._cancelAndHide))
    this._turboEvents = turboEvents

    this.current    = null
    this.timer      = null
    this.warmUntil  = 0
    this.suppressed = null
  }

  disconnect() {
    if (!this.fine) return

    document.removeEventListener("pointerover",  this._onOver)
    document.removeEventListener("pointerout",   this._onOut)
    document.removeEventListener("pointerdown",  this._onPress,       { capture: true })
    document.removeEventListener("keydown",      this._cancelAndHide, { capture: true })
    document.removeEventListener("scroll",       this._cancelAndHide, { capture: true })
    document.removeEventListener("focusin",      this._onFocusIn)
    document.removeEventListener("focusout",     this._onFocusOut)

    if (this._turboEvents) {
      this._turboEvents.forEach(ev => document.removeEventListener(ev, this._cancelAndHide))
    }

    clearTimeout(this.timer)
    this._hide()
  }

  // ── Event handlers ──────────────────────────────────────────────────────────

  _onOver(e) {
    const el = e.target.closest("[data-hint]")
    if (el === this.current) return
    if (this.current) this._leave()
    if (el) this._enter(el)
  }

  _onOut(e) {
    if (this.current && !this.current.contains(e.relatedTarget)) {
      // relatedTarget === null means pointer left the window entirely
      this._leave()
    }
  }

  _onPress() {
    clearTimeout(this.timer)
    this.suppressed = this.current
    this._hide()
  }

  _onFocusIn(e) {
    const el = e.target.closest("[data-hint]")
    if (el && el.matches(":focus-visible")) {
      this._enter(el, { instant: true })
    }
  }

  _onFocusOut(e) {
    if (e.target.closest("[data-hint]") === this.current) {
      this._leave()
    }
  }

  _cancelAndHide() {
    clearTimeout(this.timer)
    this._hide()
    this.current = null
  }

  // ── Core state ──────────────────────────────────────────────────────────────

  _enter(el, { instant } = {}) {
    if (el === this.suppressed) return
    this.current = el
    clearTimeout(this.timer)
    const delay = (instant || Date.now() < this.warmUntil) ? 0 : 400
    if (delay === 0) {
      this._show(el)
    } else {
      this.timer = setTimeout(() => this._show(el), delay)
    }
  }

  _leave() {
    clearTimeout(this.timer)
    this._hide()
    this.current    = null
    this.suppressed = null
  }

  _show(el) {
    if (!el.isConnected) return
    const content = this._contentFor(el)
    if (!content) return

    const tip   = this.tipTarget
    const label = this.labelTarget
    const keys  = this.keysTarget

    // Write content (no innerHTML with data values — build DOM nodes)
    label.textContent = content.label
    keys.innerHTML = ""
    content.keys.forEach(k => {
      const kbd = document.createElement("kbd")
      kbd.className = "hint-tip-kbd"
      kbd.textContent = k
      keys.appendChild(kbd)
    })

    // Bring into top layer before measuring so it has display
    if (typeof tip.showPopover === "function" && !tip.matches(":popover-open")) {
      tip.showPopover()
    }

    // Position (measure with left/top reset to 0 so prior position doesn't skew)
    tip.style.left = "0px"
    tip.style.top  = "0px"
    tip.classList.remove("is-on")

    const rect  = el.getBoundingClientRect()
    const tipW  = tip.offsetWidth
    const tipH  = tip.offsetHeight
    const placement = el.dataset.hintPlacement || "top"

    let x, y

    if (placement === "right") {
      x = rect.right + 8
      y = rect.top + rect.height / 2 - tipH / 2
      if (x + tipW > window.innerWidth - 8) x = rect.left - tipW - 8
    } else {
      // top (default), flip below if no room
      x = rect.left + rect.width / 2 - tipW / 2
      y = rect.top - tipH - 6
      if (y < 8) y = rect.bottom + 6
    }

    // Clamp
    x = Math.max(8, Math.min(x, window.innerWidth  - tipW - 8))
    y = Math.max(8, Math.min(y, window.innerHeight - tipH - 8))

    tip.style.left = `${x}px`
    tip.style.top  = `${y}px`
    this.shown = true

    // Add is-on on next animation frame for the CSS transition to fire
    requestAnimationFrame(() => tip.classList.add("is-on"))
  }

  // Only a tooltip that actually appeared opens the "warm" window — a pointer
  // sweeping across controls faster than the delay must not turn later ones
  // instant, or every pass over a toolbar would flash labels.
  _hide() {
    if (!this.hasTipTarget || !this.shown) return
    this.shown = false
    const tip = this.tipTarget
    tip.classList.remove("is-on")
    if (typeof tip.hidePopover === "function" && tip.matches(":popover-open")) {
      tip.hidePopover()
    }
    this.warmUntil = Date.now() + 300
  }

  // ── Content helpers ─────────────────────────────────────────────────────────

  // Determines what the tooltip should say. Returns null when the control already
  // shows the same text and has no unseen key — avoids redundant tooltips.
  _contentFor(el) {
    const label = el.dataset.hint
    const key   = el.dataset.hintKey || ""

    // Collect visible text: strip <kbd>, <svg>, .sr-only, collapse whitespace
    const visibleText = this._visibleText(el).trim()

    // Does the element already wear a visible keycap?
    const wearsCap = Array.from(el.querySelectorAll("kbd")).some(k => {
      try { return k.getClientRects().length > 0 } catch { return false }
    })

    const showKey = key !== "" && !wearsCap
    if (visibleText === label && !showKey) return null

    return { label, keys: showKey ? this._keyTokens(key) : [] }
  }

  // Text content of el with <kbd>, <svg>, and .sr-only descendants removed.
  _visibleText(el) {
    const clone = el.cloneNode(true)
    clone.querySelectorAll("kbd, svg, .sr-only").forEach(n => n.remove())
    return clone.textContent.replace(/\s+/g, " ").trim()
  }

  // Split a key string on spaces → one string per keycap.
  // Single ASCII letters are upper-cased; everything else is verbatim.
  _keyTokens(key) {
    return key.split(" ").map(token => {
      if (/^[a-z]$/.test(token)) return token.toUpperCase()
      return token
    })
  }
}
