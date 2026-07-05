# Fans a created Notification out to the recipient's registered devices as
# native push (APNs for iOS, FCM for Android).
#
# Enqueued from Notification#after_create_commit (quiet tiers are skipped there).
# No-ops cleanly when a provider isn't configured, isolates per-device failures,
# and prunes any device whose token the provider reports as dead.
class PushDeliveryJob < ApplicationJob
  queue_as :default

  def perform(notification_id)
    notification = Notification.find_by(id: notification_id)
    return unless notification

    devices = notification.user.devices.to_a
    return if devices.empty?

    payload = build_payload(notification)

    apns = Push::ApnsSender.new if Push.apns_configured? && devices.any?(&:ios?)
    fcm  = Push::FcmSender.new  if Push.fcm_configured?  && devices.any?(&:android?)

    Current.set(workspace: notification.user.workspace) do
      devices.each do |device|
        sender = device.ios? ? apns : fcm
        next unless sender # provider for this platform isn't configured

        begin
          result = sender.deliver(device, **payload)
          device.destroy if result == :invalid
        rescue => e
          Rails.logger.error("[push] delivery failed for device ##{device.id}: #{e.class}: #{e.message}")
          Sentry.capture_exception(e) if defined?(Sentry)
        end
      end
    end
  ensure
    apns&.close
  end

  private

  def build_payload(notification)
    {
      title: notification.title,
      body: notification.body,
      url: notification.link_url,
      category: notification.category
    }.compact
  end
end
