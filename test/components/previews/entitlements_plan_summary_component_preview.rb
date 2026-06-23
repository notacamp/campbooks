# frozen_string_literal: true

# The Settings → Plan page body for each tier, plus the self-hosted (unlimited)
# variant. Built from unsaved workspaces so usage counts read as zero.
class EntitlementsPlanSummaryComponentPreview < ViewComponent::Preview
  def free
    render Campbooks::Entitlements::PlanSummary.new(entitlements: resolver("free"))
  end

  def pro
    render Campbooks::Entitlements::PlanSummary.new(entitlements: resolver("pro"))
  end

  def business
    render Campbooks::Entitlements::PlanSummary.new(entitlements: resolver("business"))
  end

  def self_hosted
    render Campbooks::Entitlements::PlanSummary.new(entitlements: Entitlements::NullResolver.new)
  end

  private

  def resolver(plan)
    Entitlements::Resolver.new(Workspace.new(plan: plan))
  end
end
