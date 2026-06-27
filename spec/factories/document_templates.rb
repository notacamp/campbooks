FactoryBot.define do
  factory :document_template do
    workspace
    sequence(:name) { |n| "Template #{n}" }
    description { "A professional invoice reminder for clients." }
    html_content { "" }
    variables_schema { [] }
    ai_status { :pending }
    ai_provenance { {} }
    trait :ai_completed do
      ai_status { :completed }
      html_content { "<!DOCTYPE html><html><head><style>body{font-family:sans-serif}</style></head><body><h1>Hello {{ recipient_name }}</h1><p>Amount: {{ amount }}</p></body></html>" }
      variables_schema { [{"key"=>"recipient_name","label"=>"Recipient Name","type"=>"string","required"=>true,"default"=>""},{"key"=>"amount","label"=>"Amount","type"=>"number","required"=>false,"default"=>"0"}] }
      ai_provenance { {"provider"=>"anthropic","model"=>"claude-sonnet-4-6","region"=>"US"} }
    end
    trait :ai_processing do ai_status { :processing } end
    trait :ai_failed do ai_status { :failed } end
  end
end
