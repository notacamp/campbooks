# frozen_string_literal: true

class SettingsOverlayComponentPreview < ViewComponent::Preview
  # @label Overlay (no content — skeleton placeholder)
  def default
    render Campbooks::SettingsOverlay.new
  end

  # @label Overlay with Account section active
  def account_section
    render Campbooks::SettingsOverlay.new(current_section: "account")
  end

  # @label Overlay with Inbox section active
  def inbox_section
    render Campbooks::SettingsOverlay.new(current_section: "inbox_rules")
  end
end
