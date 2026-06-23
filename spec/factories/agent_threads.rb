FactoryBot.define do
  factory :agent_thread do
    user
    workspace { user.workspace }
    title { "Discussion" }
    purpose { :email_chat }
  end
end
