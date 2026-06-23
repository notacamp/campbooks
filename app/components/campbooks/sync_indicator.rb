# frozen_string_literal: true

module Campbooks
  # Live email-sync indicator. Renders an empty, stable placeholder while idle and
  # the shared Campbooks::StatusFeedback pill while any of the user's accounts is
  # actively scanning. EmailScanJob replaces this element in place via Turbo
  # Streams as scans start and stop; the wrapper id stays constant so it remains a
  # valid target.
  #
  # The wrapper is display:contents so, when scanning, the pill becomes a direct
  # flex item of the shared "feedback rail" (shared/_flash_toast_region) and stacks
  # with any action toasts instead of overlapping them.
  class SyncIndicator < Campbooks::Base
    DOM_ID = "sync_indicator"

    # Client-side safety net. The pill is only turned off by a Turbo broadcast; if
    # the worker dies/wedges mid-scan that broadcast never arrives. After this
    # window with no fresh broadcast the pill is definitively stale (the worker is
    # gone), so the sync_indicator Stimulus controller clears it. Mirrors the
    # server-side EmailAccount::SCAN_STALE_AFTER guard in User#email_syncing?, plus
    # a small buffer so a healthy in-flight scan is never cleared early.
    SELF_HEAL_AFTER = EmailAccount::SCAN_STALE_AFTER + 30.seconds

    # @param scanning [Boolean] whether any of the user's accounts is actively scanning
    # @param href [String] link to the sync settings / history page
    def initialize(scanning:, href: "/email_messages?inbox_settings=accounts")
      @scanning = scanning
      @href = href
    end

    def view_template
      # Own id'd wrapper (display:contents) so the pill stacks inside the feedback
      # rail. The self-heal controller lives here so its replaceChildren() clears
      # the pill back to an empty wrapper.
      div(id: DOM_ID, class: "contents", **self_heal_attrs) do
        next unless @scanning

        render Campbooks::StatusFeedback.new(
          position: :none,
          message: t(".syncing"),
          spinner: true,
          href: @href,
          animate: true
        )
      end
    end

    private

    # Only an actively-scanning pill needs the self-heal timer; the idle empty
    # wrapper has nothing to clear.
    def self_heal_attrs
      return {} unless @scanning

      {
        data: {
          controller: "sync-indicator",
          "sync-indicator-ttl-value": SELF_HEAL_AFTER.in_milliseconds
        }
      }
    end
  end
end
