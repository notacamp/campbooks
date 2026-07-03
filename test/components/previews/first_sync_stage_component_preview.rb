# frozen_string_literal: true

class FirstSyncStageComponentPreview < ViewComponent::Preview
  # The "Scout is reading your inbox" stage from the first-run home. Previews
  # pin the initial server-rendered state; in the app the first-sync Stimulus
  # controller polls and advances these states live. The status URL points at
  # nothing here, so each preview just holds its state.

  # @label Scanning (counters ticking)
  def scanning
    render_stage(state: :scanning, found: 128, sorted: 84, needs_you: 6)
  end

  # @label Just connected (waiting for the scan)
  def waiting
    render_stage(state: :waiting, found: 0, sorted: 0, needs_you: 0)
  end

  private

  def render_stage(status)
    render Campbooks::FirstSyncStage.new(
      status: status,
      status_url: "/lookbook-noop",
      inbox_path: "#",
      feed_path: "#",
      class: "mx-auto max-w-lg py-16"
    )
  end
end
