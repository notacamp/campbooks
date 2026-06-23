# frozen_string_literal: true

# Mobile bottom tab bar. It is `position: fixed` to the bottom of the preview
# frame and hidden at `lg` and up, so use a narrow viewport to see it. Scout is
# an inline Ember tab; the active tab reads in near-black ink. Secondary
# destinations (Workflows, Contacts, Activity) collapse into the "More" tab,
# which opens a popover above the bar — tap it to expand.
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

  # Attention dots on Mail/Docs (Ember) and Scout (near-black, since its icon
  # sits on an Ember chip). Home is active, so it's cleared — no dot.
  def with_badges
    render(Campbooks::BottomNav.new(items: sample_items(active: :home, badges: %i[mail scout documents])))
  end

  private

  # Mirrors the live primary nav: five dock tabs plus the three overflow
  # destinations the "More" tab folds away.
  def sample_items(active:, badges: [])
    [
      { key: :home, label: "Home", path: "#", ember: false },
      { key: :mail, label: "Mail", path: "#", ember: false },
      { key: :calendar, label: "Calendar", path: "#", ember: false },
      { key: :scout, label: "Scout", path: "#", ember: true },
      { key: :documents, label: "Docs", path: "#", ember: false },
      { key: :workflows, label: "Flows", path: "#", ember: false },
      { key: :contacts, label: "Contacts", path: "#", ember: false },
      { key: :activity, label: "Activity", path: "#", ember: false }
    ].map { |item| item.merge(active: item[:key] == active, badge: badges.include?(item[:key])) }
  end
end
