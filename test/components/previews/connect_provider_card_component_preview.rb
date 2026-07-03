# frozen_string_literal: true

class ConnectProviderCardComponentPreview < ViewComponent::Preview
  # The quiet provider connect card used on the welcome screen, the setup hub,
  # and the pre-connect home. Submits the real mailbox-connect POST, so in
  # Lookbook the buttons are inert-but-clickable (they'd 302 to OAuth).

  # @label All providers
  def all
    render_with_template(template: "connect_provider_card_component_preview/all")
  end

  # @label Compact (setup hub rows)
  def compact
    render_with_template(template: "connect_provider_card_component_preview/compact")
  end
end
