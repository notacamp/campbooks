# frozen_string_literal: true

# The product tour is a full-screen overlay driven by the `product-tour` Stimulus
# controller (normally mounted on <body>). The preview wraps it in that controller
# and auto-opens it, so all six scenes — welcome, feed, skim, reminder, scout,
# finish — are walkable here. Top-level class name to match the file path (Zeitwerk).
class ProductTourComponentPreview < ViewComponent::Preview
  # @label Full walkthrough (auto-opens)
  def default
    render_with_template
  end
end
