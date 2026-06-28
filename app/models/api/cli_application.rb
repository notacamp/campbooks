# frozen_string_literal: true

module Api
  # The well-known, first-party **public** OAuth client for the Campbooks CLI
  # (`campbooks login`). It's a public client (no secret) that must use PKCE, and
  # its client_id is identical on every deployment — cloud and self-hosted — so a
  # single CLI binary, with this UID baked in, works against any instance.
  #
  # Unlike client_credentials apps it carries no workspace/created_by: every SSO
  # token's identity comes from its resource owner (the signed-in user), resolved
  # in Api::V1::BaseController#establish_acting_identity!.
  #
  # `ensure!` is idempotent and is invoked from BOTH a data migration (so existing
  # installs get the client on deploy) and db/seeds.rb (so fresh installs, where
  # schema:load skips data migrations, still get it). Specs call it too.
  module CliApplication
    # Public, well-known client_id — safe to commit and bake into the CLI.
    UID = "campbooks-cli"
    NAME = "Campbooks CLI"

    # Loopback redirect for the browser login (RFC 8252; the port is ignored when
    # matching — see force_ssl_in_redirect_uri / URIChecker in doorkeeper.rb), plus
    # the OOB code-paste fallback for headless/`--no-browser` logins.
    REDIRECT_URIS = [ "http://127.0.0.1/callback", "urn:ietf:wg:oauth:2.0:oob" ].freeze

    module_function

    # Create or update the CLI client. Safe to call repeatedly; keeps the
    # redirect URIs and granted scope catalog in sync if they ever change.
    def ensure!
      app = Doorkeeper::Application.find_or_initialize_by(uid: UID)
      app.name = NAME
      app.redirect_uri = REDIRECT_URIS.join("\n")
      app.scopes = Api::Scopes.all.join(" ")
      app.confidential = false
      app.save!
      app
    end

    def record
      Doorkeeper::Application.find_by(uid: UID)
    end
  end
end
