# frozen_string_literal: true

class NudgeCardComponentPreview < ViewComponent::Preview
  # A passive feed nudge Scout surfaces for a stale thread.
  def default
    render Campbooks::NudgeCard.new(
      name: "Dana Whitfield",
      body: "sent the waterfront-permit note 9 days ago. You starred it, then never replied."
    )
  end
end
