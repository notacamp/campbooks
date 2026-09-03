# frozen_string_literal: true

# One segment ring on the Now page — the inbox Skim ring reframed as a filter
# over the decision deck. States: default (Ember ring), active (the current
# segment — filled disc + emphasised label), done (count 0 — gray ring + check).
class NowSegmentRingComponentPreview < ViewComponent::Preview
  def default
    render(Campbooks::Now::SegmentRing.new(segment: :mail, label: "Mail", count: 3, href: "#"))
  end

  def active
    render(Campbooks::Now::SegmentRing.new(segment: :all, label: "All", count: 5, active: true, href: "#"))
  end

  def done
    render(Campbooks::Now::SegmentRing.new(segment: :follow_ups, label: "Follow-ups", count: 0, href: "#"))
  end

  # The Docs ring is a button (opens the review overlay), so it takes no href.
  def docs_button
    render(Campbooks::Now::SegmentRing.new(segment: :docs, label: "Docs", count: 2))
  end
end
