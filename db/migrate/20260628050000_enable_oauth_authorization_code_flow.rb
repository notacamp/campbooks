# frozen_string_literal: true

# Schema changes for the public API's browser SSO grant (authorization_code +
# PKCE), used by the Campbooks CLI:
#
#   • PKCE storage — oauth_access_grants gains code_challenge /
#     code_challenge_method so Doorkeeper can verify the verifier at token
#     exchange. Without these columns PKCE is silently a no-op.
#   • Ownerless public clients — oauth_applications.workspace_id / created_by_id
#     become nullable. A public client (the CLI) has no resource owner baked in;
#     identity comes from each token's signed-in user. Confidential
#     (client_credentials) apps still require both, enforced at the model level
#     (config/initializers/doorkeeper_application_extensions.rb).
class EnableOauthAuthorizationCodeFlow < ActiveRecord::Migration[8.1]
  def up
    add_column :oauth_access_grants, :code_challenge, :string
    add_column :oauth_access_grants, :code_challenge_method, :string

    change_column_null :oauth_applications, :workspace_id, true
    change_column_null :oauth_applications, :created_by_id, true
  end

  def down
    change_column_null :oauth_applications, :created_by_id, false
    change_column_null :oauth_applications, :workspace_id, false

    remove_column :oauth_access_grants, :code_challenge_method
    remove_column :oauth_access_grants, :code_challenge
  end
end
