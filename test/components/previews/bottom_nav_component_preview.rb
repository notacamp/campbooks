# frozen_string_literal: true

# Mobile bottom tab bar. It is `position: fixed` to the bottom of the preview
# frame and hidden at `lg` and up, so use a narrow viewport to see it. Scout is
# an inline Ember tab; the active tab reads in near-black ink. Secondary
# destinations (Contacts, Activity, etc.) collapse into the "More" tab, which
# opens a popover above the bar with the overflow links and their shortcut badges.
#
# Keyboard shortcut badges: each tab and "More" menu entry carries a tiny keycap
# (the second key of the `g <key>` navigation chord). The badges are always in
# the DOM but hidden by CSS; they surface when the nav-shortcuts Stimulus
# controller sets `body[data-nav-armed]` (press `g` in the live app). To preview
# the armed state in Lookbook, open the browser console and run:
#   document.body.dataset.navArmed = "true"
class BottomNavComponentPreview < ViewComponent::Preview
  # Live items from NavigationHelper#primary_nav_items (five tabs + "More").
  def default
    render(Campbooks::BottomNav.new)
  end

  # Explicit items with Home active.
  def with_active_home
    render(Campbooks::BottomNav.new(items: sample_items(active: :home)))
  end

  # An overflow destination (Activity) is the current page, so the "More" tab
  # lights up near-black even though the active item lives inside its popover.
  def with_more_active
    render(Campbooks::BottomNav.new(items: sample_items(active: :activity)))
  end

  # Attention dots on Mail and Scout (near-black, since its icon sits on an Ember
  # chip). Home is active, so it's cleared — no dot.
  def with_badges
    render(Campbooks::BottomNav.new(items: sample_items(active: :home, badges: %i[mail scout])))
  end

  private

  # Mirrors the live primary nav: five dock tabs plus overflow destinations the
  # "More" tab folds away. Shortcut keys are included so the preview HTML has
  # the correct data-nav-shortcut-key and aria-keyshortcuts attributes.
  def sample_items(active:, badges: [])
    shortcuts = NavigationHelper::NAV_SHORTCUT_KEYS
    [
      { key: :home,          label: "Home",          path: "#", ember: false, shortcut: shortcuts[:home] },
      { key: :mail,          label: "Mail",          path: "#", ember: false, shortcut: shortcuts[:mail] },
      { key: :calendar,      label: "Calendar",      path: "#", ember: false, shortcut: shortcuts[:calendar] },
      { key: :scout,         label: "Scout",         path: "#", ember: true,  shortcut: shortcuts[:scout] },
      { key: :files,         label: "Files",         path: "#", ember: false, shortcut: shortcuts[:files] },
      { key: :workflows,     label: "Flows",         path: "#", ember: false, shortcut: shortcuts[:workflows] },
      { key: :contacts,      label: "Contacts",      path: "#", ember: false, shortcut: shortcuts[:contacts] },
      { key: :organizations, label: "Organizations", path: "#", ember: false, shortcut: shortcuts[:organizations] },
      { key: :activity,      label: "Activity",      path: "#", ember: false, shortcut: shortcuts[:activity] }
    ].map { |item| item.merge(active: item[:key] == active, badge: badges.include?(item[:key])) }
  end
end
