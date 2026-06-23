FactoryBot.define do
  factory :skim_decision do
    user
    workspace { user.workspace }
    action { "keep" }
  end
end
