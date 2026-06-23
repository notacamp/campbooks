# WebAuthn (FIDO2 / passkeys) relying-party config for the passkey second factor.
#
# `allowed_origins` must exactly match the scheme + host (+ port) the browser
# sends, and `rp_id` must be a registrable suffix of that host — we use the host
# itself, binding credentials to this exact domain. dev/test run on
# http://localhost:3000 ("localhost" is a WebAuthn-secure context over http);
# production uses https://APP_HOST. Set WEBAUTHN_ORIGIN to override (tunnel/staging).
require "webauthn"
require "uri"

origin =
  if Rails.env.production?
    ENV.fetch("WEBAUTHN_ORIGIN") { "https://#{ENV.fetch('APP_HOST', 'localhost')}" }
  else
    ENV.fetch("WEBAUTHN_ORIGIN", "http://localhost:3000")
  end

WebAuthn.configure do |config|
  config.allowed_origins = [ origin ]
  config.rp_name = "Campbooks"
  config.rp_id = URI(origin).host
end
