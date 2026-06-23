# frozen_string_literal: true

class ManagedAiCardComponentPreview < ViewComponent::Preview
  # Default: managed text + documents both available, no switch link.
  def default
    render Campbooks::ManagedAiCard.new(documents_available: true)
  end

  # With the "use my own keys instead" switch link (as shown in Settings → AI).
  def with_switch
    render Campbooks::ManagedAiCard.new(documents_available: true, show_switch: true, switch_path: "#")
  end

  # Platform has no document (vision) key — text managed, documents unavailable.
  def documents_unavailable
    render Campbooks::ManagedAiCard.new(documents_available: false)
  end
end
