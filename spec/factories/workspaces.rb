FactoryBot.define do
  factory :workspace do
    sequence(:name) { |n| "Workspace #{n}" }
    slug { name.to_s.parameterize.presence || "org-#{SecureRandom.hex(4)}" }
    settings { { "onboarding_completed_at" => Time.current.iso8601 } }
  end
end
