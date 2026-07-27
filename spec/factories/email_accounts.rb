FactoryBot.define do
  factory :email_account do
    workspace
    email_address { Faker::Internet.email }
    provider_account_id { Faker::Number.number(digits: 16).to_s }
    refresh_token { "1000.#{SecureRandom.hex(32)}" }
    active { true }

    # Password-authenticated IMAP/SMTP account (no OAuth grant).
    trait :imap do
      provider { :imap }
      refresh_token { nil }
      provider_account_id { nil }
      imap_host { "imap.example.com" }
      imap_port { 993 }
      imap_security { "ssl" }
      smtp_host { "smtp.example.com" }
      smtp_port { 587 }
      smtp_security { "starttls" }
      imap_password { "app-password" }
    end
  end
end
