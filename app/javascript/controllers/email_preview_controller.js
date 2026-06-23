import { Controller } from "@hotwired/stimulus"

// Sizes an email-preview iframe to its content and adds a height-based
// "Read more" / "Show less" toggle. The server renders the frame capped at the
// collapsed height (so even with no JS you get a clipped preview with a fade);
// on load we measure the email's real height, grow the frame to it, and reveal
// the toggle only when it overflows the cap. allow-same-origin (without
// allow-scripts) lets us read the framed document to measure — email scripts
// still cannot run.
export default class extends Controller {
  static targets = ["frame", "viewport", "button", "more", "less", "fade"]
  static values = { collapsed: String }

  connect() {
    this.expanded = false
    const doc = this.frameDoc()
    // srcdoc usually renders before connect() — measure now if it's already in.
    if (doc && doc.readyState === "complete") this.fit()
  }

  disconnect() {
    this.ro?.disconnect()
  }

  frameLoaded() {
    this.fit()
  }

  frameDoc() {
    try { return this.frameTarget.contentDocument } catch { return null }
  }

  fit() {
    const doc = this.frameDoc()
    if (!doc || !doc.body) return

    this.measure()
    // Late reflows (web fonts, images decoding) change the height — keep synced.
    if (!this.ro) {
      this.ro = new ResizeObserver(() => this.measure())
      this.ro.observe(doc.documentElement)
    }
  }

  // Grow the iframe to its content height (no inner scrollbar), then re-render.
  // body.scrollHeight is the right signal here: the srcdoc body is display:flow-root
  // so it contains its children's trailing margins (no tail clip), and — unlike
  // documentElement — it isn't inflated to the iframe's own viewport height when
  // the email is short.
  measure() {
    const doc = this.frameDoc()
    if (!doc || !doc.body) return
    const h = Math.ceil(doc.body.scrollHeight)
    if (h === this.contentHeight) return
    this.contentHeight = h
    this.frameTarget.style.height = `${h}px`
    this.render()
  }

  render() {
    const overflowing = this.contentHeight > this.collapsedPx() + 4
    this.buttonTarget.classList.toggle("hidden", !overflowing)
    const showFull = this.expanded || !overflowing
    this.viewportTarget.style.height = showFull ? `${this.contentHeight}px` : this.collapsedValue
    this.fadeTarget.classList.toggle("hidden", showFull)
  }

  collapsedPx() {
    const v = this.collapsedValue || "14rem"
    const root = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
    return v.endsWith("rem") ? parseFloat(v) * root : parseFloat(v)
  }

  toggle() {
    this.expanded = !this.expanded
    this.buttonTarget.setAttribute("aria-expanded", String(this.expanded))
    this.moreTarget.classList.toggle("hidden", this.expanded)
    this.lessTarget.classList.toggle("hidden", !this.expanded)
    this.render()
  }
}
