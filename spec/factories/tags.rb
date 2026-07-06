FactoryBot.define do
  # Legacy alias kept so existing specs that use `create(:email_tag)` continue
  # to work. Prefer `:tag` in new specs.
  factory :email_tag, class: "Tag" do
    workspace
    sequence(:name) { |n| "Tag #{n}" }
    color { "#3b82f6" }
    source { :local }
  end

  factory :tag, class: "Tag" do
    workspace
    sequence(:name) { |n| "Tag #{n}" }
    color { "#3b82f6" }
    source { :local }

    trait :external do
      email_account
      source { :external }
      sequence(:external_label_id) { |n| "label-#{n}" }
    end
  end

  factory :tag_account_link do
    tag
    email_account
    sequence(:provider_label_id) { |n| "label-#{n}" }
    provider_label_name { "Imported Label" }
  end

  factory :label_import_decision do
    email_account
    sequence(:provider_label_id) { |n| "label-#{n}" }
    provider_label_name { "Imported Label" }
    decision { :pending }
  end

  factory :tag do
    workspace
    sequence(:name) { |n| "tag-#{n}" }
    color { "#0584da" }
    source { :local }
    kind { :user }
    hidden { false }
  end
end
