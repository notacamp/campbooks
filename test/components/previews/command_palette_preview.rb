# frozen_string_literal: true

class CommandPalettePreview < ViewComponent::Preview
  def default
    render(Campbooks::CommandPalette.new)
  end
end
