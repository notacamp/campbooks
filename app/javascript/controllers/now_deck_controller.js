import { Controller } from "@hotwired/stimulus"

// The Now page's decision deck: a stack of feed cards you clear one at a time.
// Only the top card shows (CSS hides the rest). This controller gives the stack
// its motion and bookkeeping while the ACTIONS stay the feed's own — a card's
// buttons/keys/swipe still POST to /feed/items/:id/act|dismiss, which streams a
// `remove`; we just choreograph around that stream:
//
//   • fly the acted (top) card out (220ms translate + fade + slight rotate) before
//     Turbo removes it, then rise the next card in;
//   • keep the "1 of N" counter, the two peeking back sheets, and the top-card
//     focus mark right as cards leave / an Undo re-injects one on top;
//   • lazily fetch the next timeline page when the stack runs low;
//   • reveal the "Stack cleared" block the instant the last card goes.
//
// feed-keyboard is mounted alongside and acts on [data-focused]; we pin that to
// the top card and shepherd it back if a stray j/k drifts it onto a hidden card,
// so → / Enter / a letter always act on the card you can actually see. Under
// prefers-reduced-motion the animations are skipped (the global CSS guard also
// neutralises them) — cards simply swap.
export default class extends Controller {
  static targets = ["backSheet", "counter", "state", "cleared", "stack"]
  static values = {
    segment: String,
    // Kinds the active non-priority segment gathers (all/priority are handled by
    // name, so their list is empty). A live card whose kind isn't here is dropped.
    segmentKinds: { type: Array, default: [] },
    total: Number,
    url: String,
    counterFormat: { type: String, default: "{n}" }
  }

  connect() {
    this.stack = this.hasStackTarget ? this.stackTarget : this.element.querySelector("#feed_timeline")
    if (!this.stack) return

    this.reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    this.remaining = this.hasTotalValue ? this.totalValue : this.cardCount()
    this.exitDir = -1
    this.loading = false

    // Card add/remove: undo prepends (re-count + rise), load-more appends, an
    // action removes (count down + rise the next).
    this.childObserver = new MutationObserver((muts) => this.onChildMutations(muts))
    this.childObserver.observe(this.stack, { childList: true })

    // Keep the focus mark on the visible top card even if feed-keyboard drifts it.
    this.focusObserver = new MutationObserver(() => this.pinTop())
    this.focusObserver.observe(this.stack, { attributes: true, attributeFilter: ["data-focused"], subtree: true })

    // The action's direction tints the fly-out: a primary (Send/Reply) exits
    // right, an escape (Skip/Archive/dismiss) exits left.
    this.onClick = (e) => this.trackDirection(e)
    this.element.addEventListener("click", this.onClick, true)

    // Fly the top card out before Turbo removes it.
    this.onBeforeStream = (e) => this.beforeStreamRender(e)
    document.addEventListener("turbo:before-stream-render", this.onBeforeStream)

    this.pinTop()
    this.sync()
  }

  disconnect() {
    this.childObserver?.disconnect()
    this.focusObserver?.disconnect()
    this.element.removeEventListener("click", this.onClick, true)
    document.removeEventListener("turbo:before-stream-render", this.onBeforeStream)
  }

  // ── Bookkeeping ──────────────────────────────────────────────────────────

  // A card node is any element child of the stack that isn't the hidden
  // pagination-state marker (which also lives in #feed_timeline).
  isCard(node) {
    return node && node.nodeType === 1 && node.id !== "now_deck_state"
  }

  cards() {
    return Array.from(this.stack.children).filter((el) => this.isCard(el))
  }

  cardCount() {
    return this.cards().length
  }

  hasCard(id) {
    return !!(id && this.stack.querySelector(`#${CSS.escape(id)}`))
  }

  clearedEl() {
    return this.hasClearedTarget ? this.clearedTarget : this.element.querySelector("#now_deck_cleared")
  }

  // Does a live-broadcast card belong in the segment currently on screen? `all`
  // takes everything; `priority` only the attention cluster; a kind segment only
  // its own kinds. Mirrors NowController's segment split so a card can't slide into
  // a segment it wouldn't have been rendered into.
  acceptsLive(node) {
    const seg = this.segmentValue
    if (seg === "all") return true
    if (seg === "priority") return node.dataset.feedAttention === "true"
    return this.segmentKindsValue.includes(node.dataset.feedKind)
  }

  topCard() {
    return this.cards()[0] || null
  }

  // Exactly one card (the visible top) wears data-focused, so feed-keyboard's
  // → / ← / letters land on it. Idempotent: if it's already right, touch nothing
  // (so the focusObserver settles after one correction rather than looping).
  pinTop() {
    const top = this.topCard()
    this.stack.querySelectorAll("[data-focused]").forEach((el) => {
      if (el !== top) el.removeAttribute("data-focused")
    })
    if (top && top.hasAttribute("data-feed-focus-unit") && !top.hasAttribute("data-focused")) {
      top.setAttribute("data-focused", "")
    }
  }

  sync() {
    const n = this.cardCount()

    this.backSheetTargets.forEach((el) => {
      el.hidden = n < Number(el.dataset.nowDeckMin || 0)
    })

    if (this.hasCounterTarget) {
      const showable = this.remaining > 0 && n > 0
      this.counterTarget.hidden = !showable
      if (showable) this.counterTarget.textContent = this.counterFormatValue.replace("{n}", Math.max(this.remaining, 0))
    }

    if (n === 0) {
      this.revealCleared()
    } else {
      this.pinTop()
      this.maybeLoadMore(n)
    }
  }

  onChildMutations(mutations) {
    let removed = 0
    let prepended = false
    const liveAdded = []

    for (const m of mutations) {
      m.removedNodes.forEach((node) => { if (this.isCard(node)) removed += 1 })
      m.addedNodes.forEach((node) => {
        if (!this.isCard(node)) return
        // A live broadcast slid a new card into the back (or, if the deck was
        // cleared, onto the top). It's tagged so we don't confuse it with the
        // load-more append (already counted) or the undo prepend below.
        if (node.dataset && node.dataset.feedLive === "true") liveAdded.push(node)
        else if (node === this.stack.firstElementChild) prepended = true
      })
    }

    // An Undo re-injects the card on top of the deck — count it back and rise it.
    if (prepended) { this.remaining += 1; this.riseTop() }
    // Each removed card is one cleared; the newly exposed top rises in.
    if (removed) { this.remaining -= removed; this.riseTop() }

    // New live cards: count them in, and un-hide the stack if the deck was cleared.
    const wasCleared = liveAdded.length ? this.stack.hidden : false
    liveAdded.forEach((node) => {
      node.removeAttribute("data-feed-live") // consume the marker (idempotent re-renders)
      this.remaining += 1
    })
    if (wasCleared) this.reviveFromCleared()

    this.sync()

    // Motion runs AFTER sync so the back sheets are already un-hidden if the new
    // card crossed a threshold. Cleared-then-filled rises as the new top; a card
    // added behind existing ones pulses the back sheets. Either way the counter ticks.
    if (liveAdded.length) {
      if (wasCleared || this.cardCount() === liveAdded.length) this.riseTop()
      else this.pulseBack()
      this.tickCounter()
    }
  }

  // ── Motion ───────────────────────────────────────────────────────────────

  // Wrap Turbo's removal of the top card in a fly-out. Only the visible top card
  // animates; a non-top removal (or reduced motion) falls through to the instant
  // default.
  beforeStreamRender(event) {
    const stream = event.target
    if (!stream) return

    // A live-broadcast append: keep it only if the card belongs to the active
    // segment and isn't already in the deck. Otherwise drop the render entirely
    // (a no-op render never touches the DOM, so the observer never sees it).
    if (stream.getAttribute("action") === "append") { this.gateLiveAppend(event, stream); return }

    if (stream.getAttribute("action") !== "remove") return

    const id = stream.getAttribute("target")
    if (!id) return
    const card = this.stack.querySelector(`#${CSS.escape(id)}`)
    if (!card || card.parentElement !== this.stack || card !== this.topCard()) return
    if (this.reduceMotion) return

    const render = event.detail.render
    const dir = this.exitDir < 0 ? -1 : 1
    event.detail.render = (s) => {
      // Direction feeds the keyframe (see .now-fly-out in application.css).
      card.style.setProperty("--now-exit-x", `${dir * 44}px`)
      card.style.setProperty("--now-exit-rot", `${dir * 3}deg`)
      card.classList.add("now-fly-out")
      let done = false
      const finish = () => { if (!done) { done = true; render(s) } }
      card.addEventListener("animationend", finish, { once: true })
      setTimeout(finish, 320) // fallback if the animation is interrupted
    }
  }

  riseTop() {
    if (this.reduceMotion) return
    const top = this.topCard()
    if (!top) return
    top.classList.remove("now-rise")
    void top.offsetWidth // restart the animation on a card reused as the new top
    top.classList.add("now-rise")
    top.addEventListener("animationend", () => top.classList.remove("now-rise"), { once: true })
  }

  // Only the top card is visible, so a card sliding into the BACK reads as the
  // peeking back sheets pulsing once.
  pulseBack() {
    if (this.reduceMotion) return
    this.backSheetTargets.forEach((el) => {
      if (el.hidden) return
      el.classList.remove("now-back-pulse")
      void el.offsetWidth
      el.classList.add("now-back-pulse")
      el.addEventListener("animationend", () => el.classList.remove("now-back-pulse"), { once: true })
    })
  }

  // A quick scale on the "1 of N" counter as N ticks up.
  tickCounter() {
    if (this.reduceMotion || !this.hasCounterTarget || this.counterTarget.hidden) return
    this.counterTarget.classList.remove("now-counter-tick")
    void this.counterTarget.offsetWidth
    this.counterTarget.classList.add("now-counter-tick")
    this.counterTarget.addEventListener("animationend", () => this.counterTarget.classList.remove("now-counter-tick"), { once: true })
  }

  // The deck had been cleared (stack hidden, cleared block shown); a live card
  // arrived, so bring the stack back and hide the cleared moment.
  reviveFromCleared() {
    const cleared = this.clearedEl()
    if (cleared) cleared.hidden = true
    this.stack.hidden = false
  }

  // Gate an append targeting feed_timeline. For live-broadcast cards, drop when
  // the card is not for the active segment. For ALL cards (live or load-more),
  // drop when a card with the same id is already on the deck — this prevents a
  // card that arrived live from being re-appended by the next pagination page
  // (which would cause a reorder/re-animation even though Turbo's id-dedup would
  // ultimately prevent a literal duplicate).
  gateLiveAppend(event, stream) {
    if (stream.getAttribute("target") !== "feed_timeline") return
    const template = stream.querySelector("template")
    const card = template && template.content.firstElementChild
    if (!card) return
    const isLive = card.dataset.feedLive === "true"

    if ((isLive && !this.acceptsLive(card)) || this.hasCard(card.id)) {
      event.detail.render = () => {}
    }
  }

  trackDirection(event) {
    const el = event.target
    if (!(el instanceof Element)) return
    if (el.closest("[data-feed-primary]")) this.exitDir = 1
    else if (el.closest("button, a, [data-feed-dismiss], [data-feed-key]")) this.exitDir = -1
  }

  // ── Lazy load-more ───────────────────────────────────────────────────────

  maybeLoadMore(n) {
    if (this.loading || n > 3) return
    const state = this.hasStateTarget ? this.stateTarget : this.element.querySelector("#now_deck_state")
    if (!state) return
    if (state.dataset.hasMore !== "true") return
    const page = state.dataset.nextPage
    if (!page) return

    this.loading = true
    const url = new URL(this.urlValue, window.location.origin)
    url.searchParams.set("page", page)
    fetch(url, { headers: { Accept: "text/vnd.turbo-stream.html" }, credentials: "same-origin" })
      .then((r) => (r.ok ? r.text() : null))
      .then((html) => { if (html && window.Turbo) window.Turbo.renderStreamMessage(html) })
      .catch(() => {})
      .finally(() => { this.loading = false })
  }

  // ── Cleared ──────────────────────────────────────────────────────────────

  revealCleared() {
    const cleared = this.clearedEl()
    if (cleared) cleared.hidden = false
    if (this.hasCounterTarget) this.counterTarget.hidden = true
    this.backSheetTargets.forEach((el) => { el.hidden = true })
    this.stack.hidden = true
  }
}
