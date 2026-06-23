FactoryBot.define do
  # Token secrets are hashed at rest (hash_token_secrets), so the usable bearer
  # value is only available via #plaintext_token right after create — do NOT set
  # :token here, let Doorkeeper generate it (see ApiAuthHelper).
  factory :api_access_token, class: "Doorkeeper::AccessToken" do
    association :application, factory: :api_application
    expires_in { 2.hours.to_i }
    scopes { "emails:read" }
  end
end
