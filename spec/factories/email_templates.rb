FactoryBot.define do
  factory :email_template do
    workspace
    sequence(:name) { |n| "Email Template #{n}" }
    description { "A friendly welcome email for new clients." }
    subject { "" }
    body_html { "" }
    variables_schema { [] }
    ai_status { :pending }
    ai_provenance { {} }

    trait :ai_completed do
      ai_status { :completed }
      subject { "Welcome, {{ recipient_name }}!" }
      body_html { "<p>Hi {{ recipient_name }}, welcome to {{ workspace_name }}.</p>" }
      variables_schema do
        [
          { "key" => "recipient_name", "label" => "Recipient name", "type" => "text", "required" => true, "default" => "" },
          { "key" => "workspace_name", "label" => "Workspace", "type" => "text", "required" => false, "default" => "" }
        ]
      end
      ai_provenance { { "provider" => "anthropic", "model" => "claude-sonnet-4-6", "region" => "US" } }
    end

    trait :ai_processing do
      ai_status { :processing }
    end

    trait :ai_failed do
      ai_status { :failed }
    end

    # Attach completed document templates (same workspace) so applying the email
    # template produces PDF attachments.
    trait :with_documents do
      transient { documents_count { 1 } }
      after(:create) do |template, evaluator|
        create_list(:document_template, evaluator.documents_count, :ai_completed, workspace: template.workspace).each do |dt|
          template.document_templates << dt
        end
      end
    end
  end
end
