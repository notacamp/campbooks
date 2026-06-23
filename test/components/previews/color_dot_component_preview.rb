# frozen_string_literal: true

class ColorDotComponentPreview < ViewComponent::Preview
  def sm
    render(Campbooks::ColorDot.new(color: "#ef4444", size: :sm))
  end

  def md
    render(Campbooks::ColorDot.new(color: "#3b82f6"))
  end

  def lg
    render(Campbooks::ColorDot.new(color: "#22c55e", size: :lg))
  end

  def tailwind_class
    render(Campbooks::ColorDot.new(color: "accent-500", size: :lg))
  end

  def hex_colors
    render(Campbooks::ColorDotSwatches.new)
  end
end
