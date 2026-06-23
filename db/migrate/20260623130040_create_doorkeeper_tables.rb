# frozen_string_literal: true

class CreateDoorkeeperTables < ActiveRecord::Migration[8.1]
  def change
    create_table :oauth_applications do |t|
      t.string  :name,    null: false
      t.string  :uid,     null: false
      # Remove `null: false` or use conditional constraint if you are planning to use public clients.
      t.string  :secret,  null: false

      # Campbooks uses the Client Credentials flow only, which has no redirect
      # URI, so this column is nullable (per Doorkeeper's client-credentials setup).
      t.text    :redirect_uri, null: true
      t.string  :scopes,       null: false, default: ''
      t.boolean :confidential, null: false, default: true
      t.timestamps             null: false
    end

    add_index :oauth_applications, :uid, unique: true

    # NOTE: oauth_access_grants is only used by the authorization_code flow, which
    # Campbooks does not enable (client_credentials only). The table is created by
    # Doorkeeper regardless; redirect_uri is relaxed to nullable for consistency.
    create_table :oauth_access_grants do |t|
      t.references :resource_owner,  null: false
      t.references :application,     null: false
      t.string   :token,             null: false
      t.integer  :expires_in,        null: false
      t.text     :redirect_uri,      null: true
      t.string   :scopes,            null: false, default: ''
      t.datetime :created_at,        null: false
      t.datetime :revoked_at
    end

    add_index :oauth_access_grants, :token, unique: true
    add_foreign_key(
      :oauth_access_grants,
      :oauth_applications,
      column: :application_id
    )

    create_table :oauth_access_tokens do |t|
      t.references :resource_owner, index: true

      # Remove `null: false` if you are planning to use Password
      # Credentials Grant flow that doesn't require an application.
      t.references :application,    null: false

      # If you use a custom token generator you may need to change this column
      # from string to text, so that it accepts tokens larger than 255
      # characters. More info on custom token generators in:
      # https://github.com/doorkeeper-gem/doorkeeper/tree/v3.0.0.rc1#custom-access-token-generator
      #
      # t.text :token, null: false
      t.string :token, null: false

      t.string   :refresh_token
      t.integer  :expires_in
      t.string   :scopes
      t.datetime :created_at, null: false
      t.datetime :revoked_at

      # The authorization server MAY issue a new refresh token, in which case
      # *the client MUST discard the old refresh token* and replace it with the
      # new refresh token. The authorization server MAY revoke the old
      # refresh token after issuing a new refresh token to the client.
      # @see https://datatracker.ietf.org/doc/html/rfc6749#section-6
      #
      # Doorkeeper implementation: if there is a `previous_refresh_token` column,
      # refresh tokens will be revoked after a related access token is used.
      # If there is no `previous_refresh_token` column, previous tokens are
      # revoked as soon as a new access token is created.
      #
      # Comment out this line if you want refresh tokens to be instantly
      # revoked after use.
      t.string   :previous_refresh_token, null: false, default: ""
    end

    add_index :oauth_access_tokens, :token, unique: true

    # See https://github.com/doorkeeper-gem/doorkeeper/issues/1592
    if ActiveRecord::Base.connection.adapter_name == "SQLServer"
      execute <<~SQL.squish
        CREATE UNIQUE NONCLUSTERED INDEX index_oauth_access_tokens_on_refresh_token ON oauth_access_tokens(refresh_token)
        WHERE refresh_token IS NOT NULL
      SQL
    else
      add_index :oauth_access_tokens, :refresh_token, unique: true
    end

    add_foreign_key(
      :oauth_access_tokens,
      :oauth_applications,
      column: :application_id
    )

    # Uncomment below to ensure a valid reference to the resource owner's table
    # add_foreign_key :oauth_access_grants, <model>, column: :resource_owner_id
    # add_foreign_key :oauth_access_tokens, <model>, column: :resource_owner_id
  end
end
