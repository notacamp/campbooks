# frozen_string_literal: true

# The "upgrade your plan" prompt shown when an action is blocked. Three variants
# (panel / banner / inline) x three reasons (not_allowed / not_enabled / over_limit).
class EntitlementsUpgradePromptComponentPreview < ViewComponent::Preview
  # === Panel — centered, for a whole pane ===

  def panel_not_allowed
    render Campbooks::Entitlements::UpgradePrompt.new(feature: :workflows, reason: :not_allowed, variant: :panel)
  end

  def panel_over_limit
    render Campbooks::Entitlements::UpgradePrompt.new(feature: :email_accounts, reason: :over_limit, variant: :panel)
  end

  # === Banner — slim strip above a section (also what FeatureLock renders) ===

  def banner_not_allowed
    render Campbooks::Entitlements::UpgradePrompt.new(feature: :workflows, reason: :not_allowed, variant: :banner)
  end

  def banner_over_limit
    render Campbooks::Entitlements::UpgradePrompt.new(feature: :email_accounts, reason: :over_limit, variant: :banner)
  end

  # === Inline — one compact line next to a disabled control ===

  def inline_not_allowed
    render Campbooks::Entitlements::UpgradePrompt.new(feature: :managed_ai, reason: :not_allowed, variant: :inline)
  end

  def inline_over_limit
    render Campbooks::Entitlements::UpgradePrompt.new(feature: :email_accounts, reason: :over_limit, variant: :inline)
  end
end
