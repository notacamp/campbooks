# frozen_string_literal: true

# One setup step shown as a card in the decision deck (setup is a card in the
# stack, so day zero and day thirty are the same page). The raised frame comes
# from the deck's CSS; in isolation this shows the card's content.
class NowSetupCardComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Now::SetupCard.new(item: {
      key: :tags, message: "Set up your tags",
      description: "So Scout can file mail into the right streams as it arrives.",
      cta_text: "Add tags", cta_modal: false, cta_path: "#"
    }))
  end
end
