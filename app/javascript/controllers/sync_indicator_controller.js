import { Controller } from "@hotwired/stimulus"

// Self-heals the live "Syncing your inbox" pill.
//
// The pill is only ever turned *off* by a Turbo Stream broadcast from
// EmailScanJob (its ensure block, or the every-minute stale-scan reconcile). If
// the single background worker dies or wedges mid-scan — e.g. blocked on a hung
// provider request — that "off" broadcast never arrives and the pill would
// otherwise stick on screen forever.
//
// Server-side, User#email_syncing? already stops being true once a scan claim
// goes stale (EmailAccount::SCAN_STALE_AFTER), so a pill that has received no
// fresh broadcast for longer than that window is, by definition, abandoned —
// clear it. Every broadcast replaces this element, so a healthy ongoing scan
// reconnects the controller and resets the timer; only a stranded pill expires.
export default class extends Controller {
  static values = { ttl: Number }

  connect() {
    if (this.ttlValue > 0) {
      this.timeout = setTimeout(() => this.element.replaceChildren(), this.ttlValue)
    }
  }

  disconnect() {
    clearTimeout(this.timeout)
  }
}
