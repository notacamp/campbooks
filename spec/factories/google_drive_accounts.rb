FactoryBot.define do
  factory :google_drive_account do
    workspace
    email { "test@example.com" }
    refresh_token { "encrypted-refresh-token" }
    connected { true }
    scopes { GoogleDrive::OauthClient::FULL_SCOPE + " https://www.googleapis.com/auth/userinfo.email" }
  end
end
