FactoryBot.define do
  factory :bug_report do
    workspace
    user { association :user, workspace: workspace }
    description { "The Skim button does nothing when I tap it on mobile." }
    status { :open }
    page_url { "https://app.campbooks.not-a-camp.com/feed" }
    user_agent { "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36" }
    metadata do
      {
        "viewport" => "1280x800",
        "screen" => "1512x982",
        "device_pixel_ratio" => 2,
        "breakpoint" => "lg",
        "referrer" => "https://app.campbooks.not-a-camp.com/",
        "console_errors" => [],
        "locale" => "en"
      }
    end

    trait :synced do
      github_issue_number { 42 }
      github_issue_url { "https://github.com/example/campbooks/issues/42" }
    end

    trait :with_screenshot do
      after(:build) do |report|
        report.screenshot.attach(
          io: StringIO.new("fake png bytes"),
          filename: "screenshot.png",
          content_type: "image/png"
        )
      end
    end
  end
end
