# A registered native device that can receive push notifications.
#
# One row per app install: stores the platform and the provider push token
# (APNs device token for iOS, FCM registration token for Android). The native
# app registers/refreshes its token via DevicesController; PushDeliveryJob fans
# notifications out to these rows and prunes any the provider reports as dead.
class Device < ApplicationRecord
  belongs_to :user

  enum :platform, { ios: 0, android: 1 }

  validates :platform, presence: true
  validates :token, presence: true, uniqueness: true

  # Register or refresh a token for a user. A token identifies a single install,
  # so if it reappears under a different user (someone else signs in on the same
  # phone) it moves to that user rather than duplicating.
  def self.register!(user:, platform:, token:, app_version: nil)
    device = find_or_initialize_by(token: token)
    device.user = user
    device.platform = platform
    device.app_version = app_version if app_version.present?
    device.last_active_at = Time.current
    device.save!
    device
  end
end
