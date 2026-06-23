import { Controller } from "@hotwired/stimulus"

// The global "early beta / unstable" stripe (Campbooks::BetaBanner).
//
// Dismissal is remembered server-side: we set a cookie that
// ApplicationController#show_beta_banner? reads, so the banner stops rendering
// on the next request (no flash on later visits). We also remove the element
// in-place so it disappears immediately on the current page — no reload needed.
const COOKIE = "beta_banner_dismissed"
const ONE_YEAR = 60 * 60 * 24 * 365

export default class extends Controller {
  dismiss() {
    document.cookie = `${COOKIE}=1; path=/; max-age=${ONE_YEAR}; samesite=lax`
    this.element.remove()
  }
}
