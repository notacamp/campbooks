FactoryBot.define do
  factory :agent_message do
    agent_thread
    user { agent_thread.user }
    content { "A comment" }
    author_type { :user }
  end
end
