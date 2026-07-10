FactoryBot.define do
  factory :email_rule do
    workspace
    association :created_by, factory: :user
    sequence(:name) { |n| "Rule #{n}" }
    criteria { { "from" => [ "@example.com" ] } }
    archive  { false }
    mark_read { false }
    enabled  { true }
    matched_count { 0 }
  end

  factory :email_rule_run do
    association :email_rule
    workspace { email_rule.workspace }
    association :started_by, factory: :user
    status { :queued }
    matched_count   { 0 }
    processed_count { 0 }
    archived_email_ids    { [] }
    marked_read_email_ids { [] }
    moved_email_ids       { [] }
    undoable { true }
  end
end
