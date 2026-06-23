# Native push-notification configuration + provider availability.
#
# All credentials come from ENV. When a provider's vars are incomplete it is
# simply disabled and PushDeliveryJob skips that platform — so development boots
# with no push setup at all, and production can enable iOS and Android
# independently. See docs/push-notifications.md for how to obtain each value.
module Push
  module_function

  # ── Apple Push Notification service (APNs), iOS ──
  def apns_key_id      = ENV["APNS_KEY_ID"]
  def apns_team_id     = ENV["APNS_TEAM_ID"]
  def apns_bundle_id   = ENV["APNS_BUNDLE_ID"]
  def apns_key_path    = ENV["APNS_KEY_PATH"]
  def apns_environment = ENV.fetch("APNS_ENVIRONMENT", "development")

  # ── Firebase Cloud Messaging (FCM), Android ──
  def fcm_project_id       = ENV["FCM_PROJECT_ID"]
  def fcm_credentials_path = ENV["FCM_CREDENTIALS_PATH"]

  def apns_configured?
    apns_key_id.present? && apns_team_id.present? && apns_bundle_id.present? &&
      apns_key_path.present? && File.exist?(apns_key_path)
  end

  def fcm_configured?
    fcm_project_id.present? && fcm_credentials_path.present? && File.exist?(fcm_credentials_path)
  end

  # True if the provider backing the given platform ("ios"/"android") is ready.
  def configured_for?(platform)
    case platform.to_s
    when "ios"     then apns_configured?
    when "android" then fcm_configured?
    else false
    end
  end
end
