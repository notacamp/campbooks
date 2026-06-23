# frozen_string_literal: true

class ButtonComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Button.new { "Click me" })
  end

  def primary
    render(Campbooks::ButtonSizes.new(variant: :primary, label: "Primary"))
  end

  def outline
    render(Campbooks::ButtonSizes.new(variant: :outline, label: "Cancel"))
  end

  def ghost
    render(Campbooks::ButtonSizes.new(variant: :ghost, label: "Dismiss"))
  end

  def danger
    render(Campbooks::ButtonSizes.new(variant: :danger, label: "Delete"))
  end

  def as_link
    render(Campbooks::Button.new(href: "#") { "Link as button" })
  end
end
