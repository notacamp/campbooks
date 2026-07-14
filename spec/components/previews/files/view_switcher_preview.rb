# frozen_string_literal: true

module Files
  # Preview for the Files List/Grid layout switcher. The active segment is
  # synced client-side by the files-layout controller, so both segments render
  # inactive here.
  class ViewSwitcherPreview < Lookbook::Preview
    def default
      render(Campbooks::Files::ViewSwitcher.new)
    end
  end
end
