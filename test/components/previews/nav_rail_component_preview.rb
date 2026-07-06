# frozen_string_literal: true

# Desktop left navigation rail. It is `position: fixed` (pinned to the left edge
# of the preview frame) and only visible at `lg` and up, so widen the preview
# viewport to see it. Scout is the one Ember tile; the active section is a
# near-black ink pill.
#
# Keyboard shortcut badges: each nav item carries a tiny keycap (the second key
# of the `g <key>` navigation chord). The badges are always in the DOM but
# hidden by CSS; they surface when the nav-shortcuts Stimulus controller sets
# `body[data-nav-armed]` (press `g` in the live app). To preview the armed
# state, open the browser console and run:
#   document.body.dataset.navArmed = "true"
class NavRailComponentPreview < ViewComponent::Preview
  # Live items from NavigationHelper#primary_nav_items (active state follows the
  # current path, so nothing is lit inside Lookbook).
  def default
    render(Campbooks::NavRail.new)
  end

  # Explicit items with Home active, to show the active-pill + Ember-Scout
  # treatment without depending on the request path.
  def with_active_home
    render(Campbooks::NavRail.new(items: sample_items(active: :home)))
  end

  # Attention dots on Mail/Docs (Ember) and Scout (near-black, since its icon
  # sits on an Ember chip). Home is active, so it's cleared — no dot.
  def with_badges
    render(Campbooks::NavRail.new(items: sample_items(active: :home, badges: %i[mail scout])))
  end

  private

  def sample_items(active:, badges: [])
    shortcuts = NavigationHelper::NAV_SHORTCUT_KEYS
    [
      { key: :home,      label: "Home",    path: "#", ember: false, shortcut: shortcuts[:home] },
      { key: :mail,      label: "Mail",    path: "#", ember: false, shortcut: shortcuts[:mail] },
      { key: :calendar,  label: "Calendar", path: "#", ember: false, shortcut: shortcuts[:calendar] },
      { key: :scout,     label: "Scout",   path: "#", ember: true,  shortcut: shortcuts[:scout] },
      { key: :files,     label: "Files",   path: "#", ember: false, shortcut: shortcuts[:files] },
      { key: :contacts,  label: "Contacts", path: "#", ember: false, shortcut: shortcuts[:contacts] },
      { key: :activity,  label: "Activity", path: "#", ember: false, shortcut: shortcuts[:activity] }
    ].map { |item| item.merge(active: item[:key] == active, badge: badges.include?(item[:key])) }
  end
end
