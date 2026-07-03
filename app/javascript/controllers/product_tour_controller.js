import { Controller } from "@hotwired/stimulus"

// Drives the first-run product walkthrough (Campbooks::ProductTour). Mounted on
// <body> so any "Take the tour" button (data-action="product-tour#open") and the
// overlay's own controls share one controller. Everything here is sandboxed —
// no request touches the user's real data — except the one-time "seen" flag,
// POSTed on finish/skip so the tour greets the user only once.
//
// Each scene is shown one at a time. "Gated" scenes won't let the user advance
// until they complete the scene's task (tap to reveal a summary, clear the skim
// stack, set a reminder, ask Scout); completing it reveals a success cue and
// enables Next. The skim scene plays a small stack of demo cards like Stories.
const DISMISS_URL = "/tours/product_tour/dismiss"

export default class extends Controller {
  static targets = ["panel", "scene", "dot", "stepLabel", "back", "next", "footer"]

  connect() {
    this.index = 0
    this.completed = new Set()
    this.opened = false

    // Open automatically on first run (server sets data-tour-autostart on home),
    // or whenever ?tour=1 is in the URL (the replay link + a stable test handle).
    const wantsTour = new URLSearchParams(window.location.search).has("tour")
    const autostart = this.hasPanelTarget && this.panelTarget.dataset.tourAutostart === "true"
    if (wantsTour || autostart) requestAnimationFrame(() => this.open())
  }

  disconnect() {
    if (this.opened) document.documentElement.style.overflow = ""
  }

  // ── open / close ──────────────────────────────────────────────────────────

  open(event) {
    if (event) event.preventDefault()
    if (!this.hasPanelTarget) return
    this.opened = true
    this.index = 0
    this.completed.clear()
    this.resetScenes() // start every scene fresh (matters on replay)
    this.panelTarget.classList.remove("hidden")
    this.panelTarget.classList.add("flex")
    document.documentElement.style.overflow = "hidden" // freeze the page behind it
    this.render()
    this.panelTarget.focus({ preventScroll: true })
  }

  close() {
    if (!this.hasPanelTarget) return
    this.panelTarget.classList.add("hidden")
    this.panelTarget.classList.remove("flex")
    document.documentElement.style.overflow = ""
    this.opened = false
  }

  // Header "Skip tour" + finish "Explore on my own": remember it's seen, then close.
  skip(event) {
    if (event) event.preventDefault()
    this.markSeen()
    this.close()
  }

  // Finish "Connect your inbox": remember it's seen, then go connect for real.
  // With no connect path (the welcome screen — the connect cards sit right
  // behind the overlay) it just closes.
  finishConnect(event) {
    if (event) event.preventDefault()
    const path = event?.currentTarget?.dataset.tourConnectPath
    if (!path) { this.skip(); return }
    this.markSeen()
    window.location.href = path
  }

  markSeen() {
    fetch(DISMISS_URL, {
      method: "POST",
      headers: { "X-CSRF-Token": this.csrf, Accept: "application/json" },
      keepalive: true
    }).catch(() => {})
  }

  // ── navigation ──────────────────────────────────────────────────────────────

  get currentScene() { return this.sceneTargets[this.index] }
  isGated(scene) { return scene?.dataset.tourGated === "true" }

  next(event) {
    if (event) event.preventDefault()
    if (this.isGated(this.currentScene) && !this.completed.has(this.index)) return // wait for the task
    if (this.index >= this.sceneTargets.length - 1) { this.skip(); return }
    this.index += 1
    this.render()
  }

  back(event) {
    if (event) event.preventDefault()
    if (this.index === 0) return
    this.index -= 1
    this.render()
  }

  onKeydown(event) {
    if (!this.opened) return
    switch (event.key) {
      case "Escape":     this.skip(); break
      case "ArrowRight": this.next(); break
      case "ArrowLeft":  this.back(); break
      default: return
    }
    event.preventDefault()
  }

  render() {
    this.sceneTargets.forEach((scene, i) => {
      const current = i === this.index
      const entering = current && scene.classList.contains("hidden")
      scene.classList.toggle("hidden", !current)
      // Settle the scene's blocks in with a light stagger whenever it appears,
      // and retrigger its one-shot flourishes (the finale pop + glow ring).
      if (entering) {
        this.replay(scene, "tour-scene-enter")
        scene.querySelectorAll(".tour-finale-ring").forEach((el) => this.replay(el, "tour-finale-ring"))
        scene.querySelectorAll(".animate-sync-done-pop").forEach((el) => this.replay(el, "animate-sync-done-pop"))
      }
    })

    const total = this.sceneTargets.length
    this.dotTargets.forEach((dot, i) => {
      const state = i === this.index ? "current" : (i < this.index ? "done" : "ahead")
      dot.classList.toggle("w-5", state === "current")
      dot.classList.toggle("w-2", state !== "current")
      dot.classList.toggle("bg-ember-gradient", state !== "ahead")
      dot.classList.toggle("border-[1.5px]", state === "ahead")
      dot.classList.toggle("border-border", state === "ahead")
    })
    if (this.hasStepLabelTarget && this.stepLabelTarget.dataset.tmpl) {
      this.stepLabelTarget.textContent = this.stepLabelTarget.dataset.tmpl
        .replace("{current}", this.index + 1).replace("{total}", total)
    }
    if (this.hasBackTarget) this.backTarget.disabled = this.index === 0
    // The finish scene carries its own CTAs, so hide the shared footer there.
    if (this.hasFooterTarget) this.footerTarget.classList.toggle("hidden", this.index === total - 1)
    this.syncNext()
  }

  syncNext() {
    if (!this.hasNextTarget) return
    this.nextTarget.disabled = this.isGated(this.currentScene) && !this.completed.has(this.index)
  }

  // ── task completion ──────────────────────────────────────────────────────────

  // Generic "now you try" tap: stop the invite, give a tactile press, optionally
  // reveal an element (the AI summary, the reminder chip, Scout's reply), consume
  // the trigger, and mark the scene done.
  completeTask(event) {
    if (event) event.preventDefault()
    const btn = event.currentTarget
    this.stopInvite(this.currentScene)
    this.press(btn)
    if (btn.dataset.tourReveal) this.reveal(this.element.querySelector(btn.dataset.tourReveal))
    if (btn.dataset.tourConsume === "true") btn.disabled = true
    this.markDone()
  }

  // Skim scene: any keep/archive/etc. action on the visible card flies it out
  // (played like Stories) and brings up the next. When the last card is cleared,
  // the scene's done.
  skimAct(event) {
    const actionBtn = event.target.closest("[data-skim-action]")
    if (!actionBtn) return
    event.preventDefault()
    const scene = this.currentScene
    const cards = [...scene.querySelectorAll("[data-tour-skim-card]")]
    const pos = cards.findIndex((c) => !c.classList.contains("hidden"))
    if (pos === -1) return

    // Fly the card out in the direction of intent — discard left, keep right.
    const action = actionBtn.dataset.skimAction
    const left = action === "archive" || action === "block" || action === "deny"
    const card = cards[pos]
    card.style.transition = "transform .34s cubic-bezier(0.16,1,0.3,1), opacity .3s ease"
    card.style.transform = `translateX(${left ? "-" : ""}115%) rotate(${left ? "-" : ""}6deg)`
    card.style.opacity = "0"
    setTimeout(() => card.classList.add("hidden"), 340)

    const done = pos + 1
    const label = scene.querySelector("[data-tour-skim-progress]")
    if (label?.dataset.tmpl) label.textContent = label.dataset.tmpl.replace("{done}", done).replace("{total}", cards.length)

    const upcoming = cards[pos + 1]
    if (upcoming) {
      upcoming.classList.remove("hidden")
      this.replay(upcoming, "animate-fade-in")
    } else {
      this.markDone()
    }
  }

  markDone() {
    this.completed.add(this.index)
    this.stopInvite(this.currentScene)
    this.reveal(this.currentScene?.querySelector("[data-tour-done-cue]"))
    this.syncNext()
    if (this.hasNextTarget && !this.nextTarget.disabled) {
      this.nextTarget.classList.add("animate-pulse")
      setTimeout(() => this.nextTarget.classList.remove("animate-pulse"), 1400)
    }
  }

  // Reveal a hidden element with an enter animation. The codebase toggles
  // hidden↔flex explicitly (two display utilities otherwise fight by source
  // order), so honour data-tour-flex.
  reveal(el) {
    if (!el) return
    el.classList.remove("hidden")
    if (el.dataset.tourFlex === "true") el.classList.add("flex")
    this.replay(el, "animate-fade-in")
  }

  // (Re)play a one-shot animation class, retriggering it if already present.
  replay(el, klass) {
    el.classList.remove(klass)
    void el.offsetWidth
    el.classList.add(klass)
  }

  // A quick tactile press on a tapped target.
  press(el) {
    if (!el) return
    el.style.transition = "transform .12s ease"
    el.style.transform = "scale(0.97)"
    setTimeout(() => { el.style.transform = ""; el.style.transition = "" }, 130)
  }

  // Stop the looping "tap me" invites in a scene once the user has acted.
  stopInvite(scene) {
    scene?.querySelectorAll("[data-tour-invite]").forEach((el) => el.classList.remove(el.dataset.tourInvite))
  }

  // Replaying the tour starts every scene fresh: re-hide reveals, re-enable the
  // consumed triggers, restore the skim stacks + invites, reset counters.
  resetScenes() {
    this.element.querySelectorAll("[data-tour-done-cue]").forEach((el) => {
      el.classList.add("hidden")
      el.classList.remove("flex", "animate-fade-in")
    })
    this.element.querySelectorAll("[data-tour-reveal]").forEach((btn) => {
      btn.disabled = false
      const target = this.element.querySelector(btn.dataset.tourReveal)
      if (target) { target.classList.add("hidden"); target.classList.remove("flex", "animate-fade-in") }
    })
    this.element.querySelectorAll("[data-tour-consume]").forEach((btn) => { btn.disabled = false })
    this.element.querySelectorAll("[data-tour-invite]").forEach((el) => el.classList.add(el.dataset.tourInvite))
    this.sceneTargets.forEach((scene) => {
      if (scene.dataset.tourSkim !== "true") return
      const cards = [...scene.querySelectorAll("[data-tour-skim-card]")]
      cards.forEach((c, i) => {
        c.classList.toggle("hidden", i !== 0)
        c.classList.remove("animate-fade-in")
        c.style.transform = ""
        c.style.opacity = ""
        c.style.transition = ""
      })
      const label = scene.querySelector("[data-tour-skim-progress]")
      if (label?.dataset.tmpl) label.textContent = label.dataset.tmpl.replace("{done}", 0).replace("{total}", cards.length)
    })
  }

  get csrf() { return document.querySelector('meta[name="csrf-token"]')?.content || "" }
}
