# frozen_string_literal: true

class KeyboardShortcutsModalPreview < Lookbook::Preview
  def default
    render(Campbooks::KeyboardShortcutsModal.new(open: true))
  end
end
