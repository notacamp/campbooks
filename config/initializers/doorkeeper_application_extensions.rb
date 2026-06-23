# frozen_string_literal: true

# Bridges Doorkeeper's application model to Campbooks. A client_credentials token
# has no resource owner, so each API client (a Doorkeeper::Application) records
# the workspace it belongs to and the user it acts as. Api::V1::BaseController
# reads these to set Current.workspace + Current.acting_user, after which the
# app's normal permission gates (EmailMessage.accessible_to, Current.workspace.*)
# apply unchanged.
#
# Decorated in after_initialize because Doorkeeper::Application is a gem class
# (never reloaded), so a one-time decoration is correct and avoids stacking
# duplicate validators on dev code reloads. The association class names are
# strings, so nothing autoloads at boot.
Rails.application.config.after_initialize do
  Doorkeeper::Application.class_eval do
    belongs_to :workspace
    belongs_to :created_by, class_name: "User"

    validates :workspace, presence: true
    validates :created_by, presence: true

    scope :for_workspace, ->(workspace) { where(workspace: workspace) }
  end
end
