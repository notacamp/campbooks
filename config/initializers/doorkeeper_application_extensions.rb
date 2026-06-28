# frozen_string_literal: true

# Bridges Doorkeeper's application model to Campbooks. A confidential
# (client_credentials) API client has no resource owner, so it records the
# workspace it belongs to and the user it acts as; Api::V1::BaseController reads
# these to set Current.workspace + Current.acting_user, after which the app's
# normal permission gates (EmailMessage.accessible_to, Current.workspace.*) apply
# unchanged. Public clients (the first-party CLI, authorization_code + PKCE) carry
# no workspace/user — their identity comes from each token's resource owner — so
# the presence validations below are scoped to confidential apps only.
#
# Decorated in after_initialize because Doorkeeper::Application is a gem class
# (never reloaded), so a one-time decoration is correct and avoids stacking
# duplicate validators on dev code reloads. The association class names are
# strings, so nothing autoloads at boot.
Rails.application.config.after_initialize do
  Doorkeeper::Application.class_eval do
    belongs_to :workspace, optional: true
    belongs_to :created_by, class_name: "User", optional: true

    validates :workspace, presence: true, if: :confidential?
    validates :created_by, presence: true, if: :confidential?

    scope :for_workspace, ->(workspace) { where(workspace: workspace) }
  end
end
