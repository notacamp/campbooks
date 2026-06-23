import { Controller } from "@hotwired/stimulus"

// Skim-as-Stories viewer. Steps through cluster "frames" grouped into time-bucket
// rings (Priority / Today / Yesterday / This week / Earlier). Tap-right / → keeps
// the current stack (marks it addressed so it won't re-surface) and advances;
// tap-left / ← goes back; the primary "Keep" button does the same.
//
// Archive (E) archives immediately and advances, then offers an Undo. Promote (P)
// pins the stack into the Priority lane. When a cluster has several emails you can
// tick individual ones; Archive then acts on just the selection.
//
// Single-email cards load the whole email body inline (lazily, only the visible
// card). Clicking any row opens the email as a card stacked on top, where it can
// be read in full, replied to, archived, or opened in the inbox.

// Theme hue + icon are computed server-side (Campbooks::SkimTheme) and carried on
// each frame's dataset, so the viewer header matches the tray rings exactly.
const UNDO_MS = 6000

export default class extends Controller {
  static targets = ["frame", "segments", "ringIcon", "roadmap", "roadmapChip", "roadmapSep", "done", "summary", "toast", "toastIcon", "toastMessage", "undo", "emailLayer", "intro"]
  static values = { index: { type: Number, default: 0 } }

  connect() {
    this.archivedCount = 0
    this.lastArchived = []
    this.touch = null
    this.undoTimer = null
    this.render()
  }

  disconnect() {
    clearTimeout(this.undoTimer)
  }

  // ---- input ---------------------------------------------------------------

  onKeydown(event) {
    // While the first-run intro is up, nav is frozen; Enter/Space/Escape start.
    if (this.introOpen) {
      if (["Enter", " ", "Spacebar", "Escape"].includes(event.key)) { this.dismissIntro(); event.preventDefault(); event.stopPropagation() }
      return
    }
    // Never hijack real typing (e.g. the inline reply box).
    const tag = (event.target.tagName || "").toLowerCase()
    if (tag === "input" || tag === "textarea" || tag === "select" || event.target.isContentEditable) return

    // With an email card stacked on top, the action shortcuts act on THAT email
    // (so you can read it and keep archiving/pinning/replying without the mouse).
    if (this.emailOpen) {
      switch (event.key) {
        case "Escape":      this.closeEmail(); event.stopPropagation(); break
        case "e": case "E": this.archiveOpenEmail(); break
        case "p": case "P": this.promoteOpenEmail(); break
        case "r": case "R": this.toggleReply(event); break
        default: return
      }
      event.preventDefault()
      return
    }

    switch (event.key) {
      case "ArrowRight": this.next(); break
      case "ArrowLeft":  this.prev(); break
      case "e": case "E": this.archive(); break
      case "p": case "P": this.togglePriority(); break
      case "s": case "S": this.senderAction("star_sender"); break
      case "b": case "B": this.senderAction("block_sender", { advance: true }); break
      // Allow/Deny only make sense on a Pending card (whitelist gatekeeping).
      case "a": case "A": if (this.currentTheme === "pending") this.senderAction("allow_sender", { advance: true }); else return; break
      case "d": case "D":
        if (this.currentTheme === "follow_ups") this.dismissFollowUp()
        else if (this.currentTheme === "pending") this.senderAction("block_sender", { advance: true })
        else return
        break
      // Enter = "do what Scout suggests" — apply the card's learned pick, if any.
      case "Enter": if (!this.applySuggested()) return; break
      default: return // let Escape etc. bubble up to the overlay
    }
    event.preventDefault()
  }

  // Apply Scout's learned pick for the current card (the sparkle-marked primary
  // button). Returns false when the card carries no suggestion, so Enter bubbles.
  applySuggested() {
    switch (this.currentCard()?.dataset.skimSuggestedAction) {
      case "archive": this.archive(); return true
      case "promote": this.promote(); return true
      case "keep":    this.next();    return true
      default:        return false
    }
  }

  // Card action buttons (data-skim-action) bubble up to here.
  onClick(event) {
    if (this.introOpen) return
    const button = event.target.closest("[data-skim-action]")
    if (!button) return
    switch (button.dataset.skimAction) {
      case "keep":      this.next(); break
      case "archive":   this.archive(); break
      case "promote":   this.promote(); break
      case "unpromote": this.unpromote(); break
      case "dismiss_follow_up": this.dismissFollowUp(); break
      case "allow":     this.senderAction("allow_sender", { advance: true }); break
      case "deny":      this.senderAction("block_sender", { advance: true }); break
      case "block":     this.senderAction("block_sender", { advance: true }); break
      case "unblock":   this.senderAction("unblock_sender"); break
      case "star":      this.senderAction("star_sender"); break
      case "unstar":    this.senderAction("unstar_sender"); break
    }
  }

  onTouchStart(event) {
    if (this.emailOpen || this.introOpen) return
    const t = event.changedTouches[0]
    this.touch = { x: t.clientX, y: t.clientY }
  }

  onTouchEnd(event) {
    if (!this.touch) return
    const t = event.changedTouches[0]
    const dx = t.clientX - this.touch.x
    const dy = t.clientY - this.touch.y
    this.touch = null
    if (Math.abs(dx) > 40 && Math.abs(dx) > Math.abs(dy)) {
      if (dx < 0) this.next() // swipe content left → keep + advance
      else this.prev()        // swipe right → back
    }
  }

  // ---- navigation ----------------------------------------------------------

  // "Keep": mark the current stack addressed (so it won't re-surface) and advance.
  next() {
    if (this.indexValue >= this.frameTargets.length) return
    this.keepCurrent()
    this.advance()
  }

  // Pure forward nav (used after archive — that stack is gone, not "kept").
  advance() { if (this.indexValue < this.frameTargets.length) this.indexValue += 1 }

  prev() { if (this.indexValue > 0) this.indexValue -= 1 }

  // Fire-and-forget: mark the current stack's emails addressed.
  keepCurrent() {
    const ids = this.currentIds()
    if (ids.length) this.post("/skim/keep", ids).catch(() => {})
  }

  // ---- priority (promote / unpromote) --------------------------------------

  promote() {
    const ids = this.currentIds()
    if (!ids.length) return
    this.post("/skim/promote", ids)
      .then((d) => this.showToast(`Pinned ${d.promoted || ids.length} to Priority`, false))
      .catch(() => this.showToast("Couldn't pin — try again", false, false))
    this.advance()
  }

  // Retire the follow-up on this card's thread so it stops surfacing, then advance.
  dismissFollowUp() {
    const ids = this.currentIds()
    if (!ids.length) return
    this.post("/skim/dismiss_follow_up", ids)
      .then(() => this.showToast("Follow-up dismissed", false))
      .catch(() => this.showToast("Couldn't dismiss — try again", false, false))
    this.advance()
  }

  unpromote() {
    const ids = this.currentIds()
    if (!ids.length) return
    this.post("/skim/unpromote", ids)
      .then(() => this.showToast("Removed from Priority", false))
      .catch(() => this.showToast("Couldn't update — try again", false, false))
    this.advance()
  }

  // Keyboard P: trigger whichever priority action the current card offers.
  togglePriority() {
    const card = this.currentCard()
    if (!card) return
    if (card.querySelector("[data-skim-action='unpromote']")) this.unpromote()
    else this.promote()
  }

  // ---- selection -----------------------------------------------------------

  currentCard() { return this.frameTargets[this.indexValue] || null }

  currentIds() { return (this.currentCard()?.dataset.skimIds || "").split(",").filter(Boolean) }

  checkboxes(card) { return card ? [...card.querySelectorAll("[data-skim-checkbox]")] : [] }

  selectedIds(card) { return this.checkboxes(card).filter((c) => c.checked).map((c) => c.value).filter(Boolean) }

  onSelectionChange(event) {
    this.refreshSelection(event.target.closest("[data-skim-mode-target='frame']") || this.currentCard())
  }

  toggleSelectAll(event) {
    const card = event.target.closest("[data-skim-mode-target='frame']") || this.currentCard()
    this.checkboxes(card).forEach((c) => { c.checked = event.target.checked })
    this.refreshSelection(card)
  }

  refreshSelection(card) {
    const boxes = this.checkboxes(card)
    if (boxes.length === 0) return
    const selected = boxes.filter((c) => c.checked).length

    const selectAll = card.querySelector("[data-skim-select-all]")
    if (selectAll) {
      selectAll.checked = selected === boxes.length
      selectAll.indeterminate = selected > 0 && selected < boxes.length
    }
    const hint = card.querySelector("[data-skim-selection-hint]")
    if (hint) hint.textContent = selected > 0 ? `${selected} selected` : "Select all"

    const label = card.querySelector("[data-skim-action='archive'] [data-skim-label]")
    if (label) label.textContent = selected > 0 && selected < boxes.length ? `Archive ${selected}` : (boxes.length > 1 ? "Archive all" : "Archive")
  }

  // ---- archive (immediate, with undo) --------------------------------------

  archive() {
    const card = this.currentCard()
    if (!card) return
    const selected = this.selectedIds(card)
    const allIds = (card.dataset.skimIds || "").split(",").filter(Boolean)
    this.applyArchive(selected.length > 0 ? selected : allIds, card)
  }

  applyArchive(ids, card) {
    if (ids.length === 0) return
    const allIds = (card?.dataset.skimIds || "").split(",").filter(Boolean)
    const partial = !!card && ids.length < allIds.length

    // Fire the request BEFORE advancing. advance() re-renders the stack
    // (layoutFrames + a lazy turbo-frame load), which was aborting the in-flight
    // fetch so it never reached the server — the archive silently no-op'd and the
    // toast wrongly claimed failure. #keep posts first and is reliable; mirror it.
    const request = this.post("/skim/decide", ids)

    if (!partial) this.advance() // whole stack: advance optimistically (it's archived, not kept)

    request
      .then((data) => {
        const n = data.archived || ids.length
        this.archivedCount += n
        this.lastArchived = ids
        if (partial) this.removeRows(card, ids)
        this.showToast(`Archived ${n} ${n === 1 ? "email" : "emails"}`, true)
        if (this.isDone) this.renderDone()
      })
      .catch(() => {
        // Genuine failure: roll back the optimistic advance so the un-archived
        // stack comes back, and tell the truth (it really wasn't archived).
        if (!partial) this.prev()
        this.showToast("Couldn't archive — please try again", false, false)
      })
  }

  removeRows(card, ids) {
    if (!card) return
    const set = new Set(ids.map(String))
    card.querySelectorAll("[data-skim-row]").forEach((row) => {
      if (set.has(String(row.dataset.skimId))) row.remove()
    })
    const remaining = (card.dataset.skimIds || "").split(",").filter(Boolean).filter((id) => !set.has(String(id)))
    card.dataset.skimIds = remaining.join(",")
    this.updateCardCount(card, remaining.length)
    if (card.querySelectorAll("[data-skim-row]").length === 0) { this.advance(); return }
    this.refreshSelection(card)
  }

  updateCardCount(card, count) {
    const badge = card.querySelector("[data-skim-count]")
    if (badge) badge.textContent = `${count} ${count === 1 ? "email" : "emails"}`
  }

  undoLast() {
    const ids = this.lastArchived
    if (ids.length === 0) return
    this.hideToast()
    this.post("/skim/undo", ids).then((data) => {
      const n = data.restored || ids.length
      this.archivedCount = Math.max(0, this.archivedCount - n)
      this.lastArchived = []
      if (this.isDone) this.renderDone()
    })
  }

  post(url, ids) {
    return fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": this.csrf, "Accept": "application/json" },
      body: JSON.stringify({ email_ids: ids }),
      // Survive a same-tick re-render / frame load / navigation: without this the
      // browser cancels the request mid-flight and it never reaches the server.
      keepalive: true
    }).then((r) => (r.ok ? r.json() : Promise.reject(r)))
  }

  get csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }

  // ---- sender actions (star / block / allow / deny) ------------------------

  // Run a sender-scoped registry action on the current card's sender. `advance`
  // steps past the card (allow/deny/block resolve it); otherwise it stays put.
  senderAction(tool, { advance = false } = {}) {
    const ids = this.currentIds()
    if (!ids.length) return
    if (advance) this.advance()
    this.postSender(tool, ids)
      .then((d) => this.showToast(d.message || "Done", false))
      .catch(() => this.showToast("Couldn't update — try again", false, false))
  }

  postSender(tool, ids) {
    return fetch("/skim/sender_action", {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": this.csrf, "Accept": "application/json" },
      body: JSON.stringify({ tool, email_ids: ids })
    }).then((r) => (r.ok ? r.json() : Promise.reject(r)))
  }

  get currentTheme() { return this.currentCard()?.dataset.skimTheme || null }

  // ---- stacked email card --------------------------------------------------

  get emailOpen() { return this.hasEmailLayerTarget && !this.emailLayerTarget.classList.contains("hidden") }

  openEmail() {
    if (!this.hasEmailLayerTarget) return
    this.emailLayerTarget.classList.remove("hidden")
    this.emailLayerTarget.classList.add("flex")
  }

  closeEmail(event) {
    if (event) event.preventDefault()
    if (!this.hasEmailLayerTarget) return
    this.emailLayerTarget.classList.add("hidden")
    this.emailLayerTarget.classList.remove("flex")
  }

  archiveEmail(event) {
    const id = event.currentTarget?.dataset.skimId
    const card = this.currentCard()
    this.closeEmail()
    if (id) this.applyArchive([id], card)
  }

  // The id of the email currently shown in the stacked card (read off its action
  // buttons), so keyboard shortcuts can act on it.
  openEmailId() {
    return this.emailLayerTarget?.querySelector("[data-skim-id]")?.dataset.skimId || null
  }

  archiveOpenEmail() {
    const id = this.openEmailId()
    const card = this.currentCard()
    this.closeEmail()
    if (id) this.applyArchive([id], card)
  }

  promoteOpenEmail() {
    const id = this.openEmailId()
    if (!id) return
    this.post("/skim/promote", [id])
      .then(() => this.showToast("Pinned to Priority", false))
      .catch(() => this.showToast("Couldn't pin — try again", false, false))
    this.closeEmail()
  }

  toggleReply(event) {
    if (event) event.preventDefault()
    const box = this.emailLayerTarget?.querySelector("[data-skim-reply]")
    if (!box) return
    box.classList.toggle("hidden")
    if (!box.classList.contains("hidden")) box.querySelector("[data-skim-reply-body]")?.focus()
  }

  sendReply(event) {
    const btn = event.currentTarget
    const id = btn?.dataset.skimId
    if (!id) return
    const textarea = this.emailLayerTarget?.querySelector("[data-skim-reply-body]")
    const body = textarea ? textarea.value.trim() : ""
    if (!body) { textarea?.focus(); return }

    btn.disabled = true
    const original = btn.textContent
    btn.textContent = "Sending…"
    fetch(`/skim/email/${id}/reply`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": this.csrf, "Accept": "application/json" },
      body: JSON.stringify({ body })
    })
      .then((r) => (r.ok ? r.json() : Promise.reject(r)))
      .then(() => { this.closeEmail(); this.showToast("Reply sent", false) })
      .catch(() => { btn.disabled = false; btn.textContent = original; this.showToast("Couldn't send reply", false, false) })
  }

  // ---- first-run intro -----------------------------------------------------

  get introOpen() { return this.hasIntroTarget && !this.introTarget.classList.contains("hidden") }

  // "Start": hide the explainer, hand keyboard control back to the stack, and
  // record the tour as seen so it won't greet this user again. Idempotent.
  dismissIntro(event) {
    if (event) event.preventDefault()
    if (!this.introOpen) return
    this.introTarget.classList.add("hidden")
    this.introTarget.classList.remove("flex")
    this.element.focus({ preventScroll: true })
    const key = this.introTarget.dataset.tourKey
    if (key) this.markTourSeen(key)
  }

  // Re-open the explainer (header "?"), e.g. for someone who skipped it.
  showIntro(event) {
    if (event) event.preventDefault()
    if (!this.hasIntroTarget) return
    this.introTarget.classList.remove("hidden")
    this.introTarget.classList.add("flex")
  }

  markTourSeen(key) {
    fetch(`/tours/${encodeURIComponent(key)}/dismiss`, {
      method: "POST",
      headers: { "X-CSRF-Token": this.csrf, "Accept": "application/json" }
    }).catch(() => {})
  }

  // ---- toast ---------------------------------------------------------------

  showToast(message, undoable, success = true) {
    if (!this.hasToastTarget) return
    if (this.hasToastMessageTarget) this.toastMessageTarget.textContent = message
    if (this.hasToastIconTarget) this.toastIconTarget.classList.toggle("hidden", !success)
    if (this.hasUndoTarget) this.undoTarget.classList.toggle("hidden", !undoable)
    this.toastTarget.classList.remove("hidden")
    this.toastTarget.classList.add("flex")
    clearTimeout(this.undoTimer)
    this.undoTimer = setTimeout(() => this.hideToast(), UNDO_MS)
  }

  hideToast() {
    clearTimeout(this.undoTimer)
    if (!this.hasToastTarget) return
    this.toastTarget.classList.add("hidden")
    this.toastTarget.classList.remove("flex")
  }

  // ---- render --------------------------------------------------------------

  get isDark() { return document.documentElement.classList.contains("dark") }

  get isDone() { return this.frameTargets.length === 0 || this.indexValue >= this.frameTargets.length }

  indexValueChanged() { this.render() }

  render() {
    this.closeEmail() // navigating the stack dismisses any open email card
    this.layoutFrames()
    if (this.hasDoneTarget) {
      this.doneTarget.classList.toggle("hidden", !this.isDone)
      this.doneTarget.classList.toggle("flex", this.isDone)
    }
    if (this.isDone) {
      this.tint(276); this.setHeaderIcon(null); this.renderDone()
      // Reached the end of the last ring → tell the "Skim all" orchestrator (home)
      // it may hand off into document review. Fired once; reset below when we leave
      // the done-state so stepping back then forward re-arms it.
      if (!this.completedFired) { this.completedFired = true; this.dispatch("completed") }
      return
    }
    this.completedFired = false

    const frame = this.frameTargets[this.indexValue]
    const pos = parseInt(frame.dataset.skimPos, 10) || 1
    const ringTotal = parseInt(frame.dataset.skimTotal, 10) || 1
    const hue = parseInt(frame.dataset.skimHue, 10) || 276
    this.buildSegments(ringTotal, pos, hue)
    this.tint(hue)
    this.setHeaderIcon(frame.dataset.skimIcon)
    this.updateRoadmap(parseInt(frame.dataset.skimRingIndex, 10) || 0, pos)
    this.refreshSelection(frame)
    this.loadCurrentBody(frame)
  }

  // Roadmap of theme rings in the header: emphasise the current bucket (with its
  // live position), hide the ones already done, and leave the rest faded — so the
  // buckets still approaching stay visible next to the current one, scrolled to
  // the left.
  updateRoadmap(currentRingIndex, pos) {
    if (!this.hasRoadmapChipTarget) return
    this.roadmapChipTargets.forEach((chip) => {
      const idx = parseInt(chip.dataset.skimChipIndex, 10)
      const total = parseInt(chip.dataset.skimChipTotal, 10) || 1
      const isCurrent = idx === currentRingIndex
      chip.classList.toggle("hidden", idx < currentRingIndex)
      chip.classList.toggle("font-semibold", isCurrent)
      chip.classList.toggle("text-foreground", isCurrent)
      chip.classList.toggle("font-medium", !isCurrent)
      chip.classList.toggle("text-muted-foreground/40", !isCurrent)
      const counter = chip.querySelector("[data-skim-roadmap-counter]")
      if (counter) {
        counter.textContent = isCurrent ? `${pos} / ${total}` : String(total)
        counter.classList.toggle("rounded-full", isCurrent)
        counter.classList.toggle("bg-foreground/10", isCurrent)
        counter.classList.toggle("text-muted-foreground", isCurrent)
        counter.classList.toggle("text-muted-foreground/40", !isCurrent)
      }
    })
    // Chevrons show only ahead of the current bucket (never before the leftmost one).
    this.roadmapSepTargets.forEach((sep) => {
      sep.classList.toggle("hidden", parseInt(sep.dataset.skimSepIndex, 10) <= currentRingIndex)
    })
    if (this.hasRoadmapTarget) this.roadmapTarget.scrollLeft = 0
  }

  // Auto-load the body of a single-email card when it becomes the current frame
  // (only the visible card; only the one marked data-skim-autoload, so multi-email
  // rows stay collapsed until expanded).
  loadCurrentBody(frame) {
    const lazy = frame.querySelector("turbo-frame[data-skim-autoload]:not([src])")
    if (lazy) lazy.setAttribute("src", lazy.dataset.skimBodySrc)
  }

  // Expand / collapse an email row to read its WHOLE body inline (no stacked card,
  // no "open in full" needed). The body loads lazily on the first expand.
  toggleRow(event) {
    const header = event.currentTarget
    const row = header.closest("[data-skim-row]")
    const panel = row && row.querySelector("[data-skim-body]")
    if (!panel) return
    const opening = panel.classList.contains("hidden")
    panel.classList.toggle("hidden")
    header.setAttribute("aria-expanded", String(opening))
    header.querySelector("[data-skim-chevron]")?.classList.toggle("rotate-180", opening)
    header.querySelector("[data-skim-collapsed]")?.classList.toggle("hidden", opening)
    if (opening) {
      const frame = panel.querySelector("turbo-frame[data-skim-body-src]:not([src])")
      if (frame) frame.setAttribute("src", frame.dataset.skimBodySrc)
    }
  }

  setHeaderIcon(svg) {
    if (!this.hasRingIconTarget) return
    this.ringIconTarget.innerHTML = svg || ""
    this.ringIconTarget.classList.toggle("hidden", !svg)
  }

  layoutFrames() {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    const trans = reduce ? "" : "transition:opacity .28s ease;"
    this.frameTargets.forEach((f, i) => {
      if (i === this.indexValue) {
        f.classList.remove("hidden")
        f.style.cssText = trans + "opacity:1;z-index:10;pointer-events:auto;"
      } else {
        f.classList.add("hidden")
        f.style.cssText = trans + "opacity:0;pointer-events:none;"
      }
    })
  }

  buildSegments(total, pos, hue) {
    if (!this.hasSegmentsTarget) return
    this.segmentsTarget.replaceChildren()
    const track = this.isDark ? "oklch(30% 0.01 60)" : "oklch(91% 0.004 60)"
    const current = this.isDark ? "oklch(97% 0.003 60)" : "oklch(20% 0.006 60)"
    const filled = current
    const MAX_SEGMENTS = 16

    if (total > MAX_SEGMENTS) {
      const bar = document.createElement("div")
      bar.style.cssText = `flex:1 1 0;height:3px;border-radius:9999px;overflow:hidden;background:${track}`
      const fill = document.createElement("div")
      fill.style.cssText = `height:100%;border-radius:9999px;transition:width .25s cubic-bezier(0.16,1,0.3,1);width:${Math.round((pos / total) * 100)}%;background:${current}`
      bar.appendChild(fill)
      this.segmentsTarget.appendChild(bar)
      return
    }

    for (let i = 1; i <= total; i++) {
      const seg = document.createElement("div")
      const color = i < pos ? filled : i === pos ? current : track
      seg.style.cssText = `flex:1 1 0;height:3px;border-radius:9999px;transition:background-color .25s cubic-bezier(0.16,1,0.3,1);background:${color}`
      this.segmentsTarget.appendChild(seg)
    }
  }

  tint(hue) {
    this.element.style.background = this.isDark
      ? "oklch(16% 0.005 60)"
      : "oklch(98.5% 0.002 60)"
  }

  renderDone() {
    const n = this.archivedCount
    if (this.hasSummaryTarget) {
      this.summaryTarget.textContent = n > 0
        ? `${n} email${n === 1 ? "" : "s"} archived. The rest are still in your inbox.`
        : "You went through every stack. Inbox handled."
    }
  }
}
