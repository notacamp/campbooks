# frozen_string_literal: true

class BetaBannerComponentPreview < ViewComponent::Preview
  # The global cloud beta stripe. Rendered in the app shell behind
  # `show_beta_banner?` (cloud-only, dismissible). The dismiss button drops a
  # cookie and removes the stripe for the current page.
  def default
    render(Campbooks::BetaBanner.new)
  end
end
