# frozen_string_literal: true

# The current-plan chip (Free / Pro / Business / Unlimited), in both sizes.
class EntitlementsPlanBadgeComponentPreview < ViewComponent::Preview
  def free
    render Campbooks::Entitlements::PlanBadge.new(plan: "free")
  end

  def pro
    render Campbooks::Entitlements::PlanBadge.new(plan: "pro")
  end

  def business
    render Campbooks::Entitlements::PlanBadge.new(plan: "business")
  end

  def unlimited
    render Campbooks::Entitlements::PlanBadge.new(plan: "unlimited")
  end

  def small
    render Campbooks::Entitlements::PlanBadge.new(plan: "pro", size: :sm)
  end
end
