# frozen_string_literal: true

# Bridges a Doorkeeper application (a customer's API client) to the Campbooks
# workspace it belongs to and the user it "acts as". The client_credentials grant
# issues an application-scoped token with no resource owner, so
# Api::V1::BaseController resolves Current.workspace + Current.acting_user from
# these columns (see config/initializers/doorkeeper_application_extensions.rb).
class AddWorkspaceAndCreatorToOauthApplications < ActiveRecord::Migration[8.1]
  def change
    add_reference :oauth_applications, :workspace, null: false, foreign_key: true
    add_reference :oauth_applications, :created_by, null: false,
                  foreign_key: { to_table: :users }
  end
end
