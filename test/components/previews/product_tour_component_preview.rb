# frozen_string_literal: true

# The product tour is a full-screen overlay driven by the `product-tour` Stimulus
# controller (normally mounted on <body>). The preview wraps it in that controller
# and auto-opens it, so all slides can be walked through in Lookbook.
#
# Walkthrough v2: six slides (or five when Tasks is disabled) — intro with
# rotating Scout statements, Inbox, Calendar, Tasks (feature-gated), Documents,
# and "Much more" with docs + connect CTA.
class ProductTourComponentPreview < ViewComponent::Preview
  # @label Full walkthrough (auto-opens)
  def default
    render_with_template
  end
end
