# frozen_string_literal: true

# FeatureLock renders its block when the plan grants the feature, otherwise an
# UpgradePrompt banner. These previews show the locked state (the meaningful
# visual); when granted it simply renders the wrapped content.
class EntitlementsFeatureLockComponentPreview < ViewComponent::Preview
  def locked_workflows
    render Campbooks::Entitlements::FeatureLock.new(
      feature: :workflows, entitlements: free_entitlements, reason: :not_allowed
    )
  end

  def locked_over_limit
    render Campbooks::Entitlements::FeatureLock.new(
      feature: :email_accounts, entitlements: capped_entitlements, reason: :over_limit
    )
  end

  private

  def free_entitlements
    Entitlements::Resolver.new(Workspace.new(plan: "free"))
  end

  # email_accounts is allowed on free, but with overrides forced off to demo the lock.
  def capped_entitlements
    Entitlements::Resolver.new(
      Workspace.new(plan: "free", entitlement_overrides: { "email_accounts" => { "enabled" => false } })
    )
  end
end
