# frozen_string_literal: true

# Stops an action the current workspace's plan doesn't allow (or whose usage limit
# is reached) and tells the user how to unblock it (upgrade / manage the plan).
#
# The rich, inline locked states (Campbooks::Entitlements::FeatureLock /
# UpgradePrompt) are shown proactively by the views, so the user normally never
# reaches a doomed submit. This is the safety net for a direct POST (a stale page,
# an API client) so the request never reaches a model/job/provider. Mirrors
# AiProviderGuard. Use it as a guard clause:
#
#   def create
#     return if require_entitlement!(:workflows)
#     # …proceed…
#   end
module EntitlementGuard
  extend ActiveSupport::Concern

  private

  # Renders an "upgrade your plan" response and returns true when <feature_key> is
  # not available (not allowed, toggled off, or at its limit) for the current
  # workspace; returns false (and does nothing) when the action may proceed.
  #
  # Pass ignore_limit: true to gate only *access* (plan not allowed / toggled
  # off) while letting an over-limit workspace through — e.g. so a free user who
  # is at their pipeline cap can still view, edit, and delete the ones they have.
  # Reserve the full check (default) for the create action.
  def require_entitlement!(feature_key, ignore_limit: false)
    reason = current_entitlements.allow?(feature_key.to_sym)
    return false if reason == :ok
    return false if ignore_limit && reason == :over_limit

    message = entitlement_block_message(feature_key, reason)

    respond_to do |format|
      format.turbo_stream { render turbo_stream: notify_stream(message, severity: :warning) }
      format.json do
        render json: { error: "entitlement_blocked", feature: feature_key.to_s, reason: reason.to_s },
               status: :payment_required
      end
      format.any { redirect_back fallback_location: root_path, warning: message }
    end
    true
  end

  # Human-facing "why you're blocked" copy, keyed by the reason
  # (:not_allowed / :not_enabled / :over_limit).
  def entitlement_block_message(feature_key, reason)
    feature = t("entitlements.features.#{feature_key}")
    t("entitlements.blocked.#{reason}", feature: feature, plan: current_entitlements.plan_name)
  end
end
