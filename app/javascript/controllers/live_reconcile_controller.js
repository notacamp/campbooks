import { Controller } from "@hotwired/stimulus"
// The importmap pins "@hotwired/turbo-rails" (which re-exports Turbo), not
// bare "@hotwired/turbo" -- importing the latter fails to resolve in the browser.
import { Turbo } from "@hotwired/turbo-rails"

// Handles real-time stream reconciliation on reconnect and backgrounded-tab
// recovery for any surface that subscribes via turbo_stream_from.
//
// Action Cable does NOT replay missed broadcasts. A client whose socket drops
// (laptop sleep, network blip, backgrounded tab) misses any broadcasts sent
// during the gap and its DOM silently drifts from the server.
//
// This controller detects a genuine reconnect by watching the `connected`
// attribute on every turbo-cable-stream-source within its element (via a
// MutationObserver), then does a targeted refresh:
//
//   - Frame-aware: if this.element is inside a <turbo-frame>, it calls
//     frame.reload() so only that fragment refreshes (the Scout overlay
//     reloads just its conversation frame, not the whole page).
//   - Otherwise: Turbo.visit(window.location.href, { action: "replace" }).
//
// Backgrounded-tab recovery: if the socket reconnects while the tab is
// hidden, we set a dirty flag and refresh once the tab becomes visible.
//
// Lazily-added stream sources (e.g. from lazy-loaded frames) are picked up
// via a child MutationObserver. Both observers disconnect on controller
// teardown to avoid leaks.

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

  // -- Private --

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
      }
      // Disconnected -- no action; _wasConnected marks that the next
      // re-connect is a genuine reconnect, not the first page load.
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
      const frame = this.element.closest("turbo-frame")
      if (frame) {
        frame.reload()
      } else {
        Turbo.visit(window.location.href, { action: "replace" })
      }
    }, DEBOUNCE_MS)
  }
}
