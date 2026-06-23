# frozen_string_literal: true

# Desktop left navigation rail. It is `position: fixed` (pinned to the left edge
# of the preview frame) and only visible at `lg` and up, so widen the preview
# viewport to see it. Scout is the one Ember tile; the active section is a
# near-black ink pill.
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
    render(Campbooks::NavRail.new(items: sample_items(active: :home, badges: %i[mail scout documents])))
  end

  private

  def sample_items(active:, badges: [])
    [
      { key: :home, label: "Home", path: "#", ember: false },
      { key: :mail, label: "Mail", path: "#", ember: false },
      { key: :scout, label: "Scout", path: "#", ember: true },
      { key: :documents, label: "Docs", path: "#", ember: false },
      { key: :workflows, label: "Flows", path: "#", ember: false }
    ].map { |item| item.merge(active: item[:key] == active, badge: badges.include?(item[:key])) }
  end
end
