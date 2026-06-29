# frozen_string_literal: true

class InboxViewSwitcherComponentPreview < ViewComponent::Preview
  # The bare segmented control. The active segment and switching behaviour are
  # applied at runtime by the `inbox-layout` Stimulus controller, so rendered in
  # isolation no segment looks selected — see `interactive` for the live control.
  def default
    render(Campbooks::InboxViewSwitcher.new)
  end

  # Wrapped in the `inbox-layout` controller so the active segment highlights and
  # clicking switches Default / List / Board (persists to localStorage).
  def interactive
    switcher = render(Campbooks::InboxViewSwitcher.new)
    %(<div data-controller="inbox-layout" class="p-6">#{switcher}</div>).html_safe
  end

  # board: false — Default + List only, used on the standalone inbox index (which
  # has no Board pane to switch into).
  def without_board
    render(Campbooks::InboxViewSwitcher.new(board: false))
  end
end
