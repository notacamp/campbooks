# frozen_string_literal: true

class CommandPalettePreview < Lookbook::Preview
  # The Cmd+K command palette, opened with representative navigation/action
  # commands and live search results (emails, contacts) grouped by type.
  # In the running app the list is rendered live by the Stimulus controller.
  def default
    render(Campbooks::CommandPalette.new(preview: true))
  end

  # A composite command mid-flow: "Move email to folder" with the email picked
  # (breadcrumb) and the destination-folder picker active.
  def capture
    render(Campbooks::CommandPalette.new(preview: :capture))
  end
end
