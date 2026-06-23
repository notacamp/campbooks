# frozen_string_literal: true

class SyncIndicatorPreview < Lookbook::Preview
  # Idle — nothing is scanning, so the indicator collapses to an empty slot.
  def idle
    render(Campbooks::SyncIndicator.new(scanning: false))
  end

  # Scanning — a scan is in progress; spinner + label, links to sync history.
  def scanning
    render(Campbooks::SyncIndicator.new(scanning: true))
  end
end
