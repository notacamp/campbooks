# frozen_string_literal: true

# Previews for the AI data-region pill — EU (green) vs non-EU (amber) vs an
# unknown provider (renders nothing).
class AiRegionBadgePreview < ViewComponent::Preview
  def eu
    render Campbooks::AiRegionBadge.new(provider: "mistral")
  end

  def us
    render Campbooks::AiRegionBadge.new(provider: "openai")
  end

  def china
    render Campbooks::AiRegionBadge.new(provider: "deepseek")
  end

  def unknown_provider
    render Campbooks::AiRegionBadge.new(provider: "nonexistent")
  end
end
