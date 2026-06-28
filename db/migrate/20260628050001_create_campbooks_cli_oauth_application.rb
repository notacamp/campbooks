# frozen_string_literal: true

# Creates the well-known, first-party public OAuth client for the Campbooks CLI
# (`campbooks login`, authorization_code + PKCE — see Api::CliApplication).
#
# Data-only: it inserts/updates one oauth_applications row and changes no schema.
# Existing installs get the client here on deploy; fresh installs get it from
# db/seeds.rb, since `schema:load` (used for brand-new databases) does not replay
# data migrations. Depends on AllowOwnerlessOauthApplications (this client has no
# workspace/created_by).
class CreateCampbooksCliOauthApplication < ActiveRecord::Migration[8.1]
  def up
    Api::CliApplication.ensure!
  end

  def down
    Doorkeeper::Application.where(uid: Api::CliApplication::UID).destroy_all
  end
end
