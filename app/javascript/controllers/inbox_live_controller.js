import { Controller } from "@hotwired/stimulus"
// The importmap pins "@hotwired/turbo-rails" (which re-exports Turbo), not
// bare "@hotwired/turbo" — importing the latter fails to resolve in the browser.
import { Turbo } from "@hotwired/turbo-rails"

// Handles real-time inbox refresh in two scenarios:
//
//   1. WebSocket reconnect — when a Turbo cable stream source goes from
//      disconnected back to connected after an initial connect, the list may
//      have missed broadcasts. We detect the reconnect via a MutationObserver
//      on the `connected` attribute of all turbo-cable-stream-source elements
//      within our scope and do a full-page replace.
//
//   2. "New mail" pill click — the `inbox-live#refresh` action lets the pill
//      broadcasted by Emails::InboxBroadcaster trigger the same refresh so the
//      user gets the inserted row on filtered views (folder/group/search).
//
// Both paths share the same debounced Turbo.visit call so overlapping triggers
// coalesce into one navigation.

const DEBOUNCE_MS = 400

export default class extends Controller {
  connect() {
    this._connectedSources = new WeakSet()
    this._dirty = false
    this._debounceTimer = null

    this._observer = new MutationObserver(this._onMutation.bind(this))

    // Observe all turbo-cable-stream-source elements already in scope.
    this._attachSources()

    // Watch for new stream sources added after connect (e.g. lazy-loaded frames).
    this._childObserver = new MutationObserver(() => this._attachSources())
    this._childObserver.observe(this.element, { childList: true, subtree: true })

    this._onVisibilityChange = this._handleVisibilityChange.bind(this)
    document.addEventListener("visibilitychange", this._onVisibilityChange)
  }

  disconnect() {
    this._observer.disconnect()
    this._childObserver.disconnect()
    document.removeEventListener("visibilitychange", this._onVisibilityChange)
    clearTimeout(this._debounceTimer)
  }

  // Action target: data-action="click->inbox-live#refresh"
  refresh() {
    this._scheduleRefresh()
  }

  // ── Private ──────────────────────────────────────────────────────────────

  _attachSources() {
    this.element.querySelectorAll("turbo-cable-stream-source").forEach((el) => {
      if (this._connectedSources.has(el)) return
      this._connectedSources.add(el)
      this._observer.observe(el, { attributes: true, attributeFilter: ["connected"] })
    })
  }

  _onMutation(mutations) {
    for (const mutation of mutations) {
      const el = mutation.target
      const nowConnected = el.hasAttribute("connected")

      if (nowConnected) {
        if (el._wasConnected) {
          // Reconnect detected (was connected, went away, came back).
          if (document.visibilityState === "visible") {
            this._scheduleRefresh()
          } else {
            this._dirty = true
          }
        }
        el._wasConnected = true
      } else {
        // Disconnected — no action, but presence of _wasConnected flag marks
        // that a future re-connect is a reconnect (not first load).
      }
    }
  }

  _handleVisibilityChange() {
    if (document.visibilityState === "visible" && this._dirty) {
      this._dirty = false
      this._scheduleRefresh()
    }
  }

  _scheduleRefresh() {
    clearTimeout(this._debounceTimer)
    this._debounceTimer = setTimeout(() => {
      Turbo.visit(window.location.href, { action: "replace" })
    }, DEBOUNCE_MS)
  }
}
