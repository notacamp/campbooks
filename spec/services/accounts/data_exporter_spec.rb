require "rails_helper"

RSpec.describe Accounts::DataExporter do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, name: "Dana Subject") }

  it "exports the user's own personal data as a structured hash" do
    create(:session, user: user, ip_address: "203.0.113.7", user_agent: "Firefox")
    thread = create(:agent_thread, user: user, workspace: workspace, title: "Help me")
    create(:agent_message, agent_thread: thread, user: user, author_type: :user, content: "hi")

    data = described_class.new(user).as_json

    expect(data[:account]).to include(email_address: user.email_address, name: "Dana Subject")
    expect(data[:account]).to have_key(:terms_accepted_at)
    expect(data[:sessions].first).to include(ip_address: "203.0.113.7", user_agent: "Firefox")
    expect(data[:ai_conversations].first[:title]).to eq("Help me")
    expect(data[:ai_conversations].first[:messages].first).to include(author: "user", content: "hi")
    expect(data[:meta][:subject]).to eq(user.email_address)
  end

  it "lists only the email accounts the user can read" do
    readable = create(:email_account, workspace: workspace)
    create(:email_account_user, user: user, email_account: readable, can_read: true)
    create(:email_account, workspace: workspace) # not shared with the user

    emails = described_class.new(user).as_json[:connected_email_accounts]

    expect(emails.map { |e| e[:email_address] }).to eq([ readable.email_address ])
  end

  it "produces valid JSON" do
    expect { JSON.parse(described_class.new(user).to_json) }.not_to raise_error
  end
end
