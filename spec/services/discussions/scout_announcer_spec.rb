# frozen_string_literal: true

require "rails_helper"

RSpec.describe Discussions::ScoutAnnouncer do
  let(:workspace) { Workspace.create!(name: "Announcer WS") }
  let(:user) { create_user("owner@example.com") }
  let(:account) do
    acct = EmailAccount.create!(
      workspace: workspace, email_address: "mailbox@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    acct.email_account_users.create!(user: user, owner: true, can_read: true)
    acct
  end
  let(:thread) { account.email_threads.create!(subject: "Quarterly report") }
  let(:message) do
    account.email_messages.create!(
      email_thread: thread, provider_message_id: "m-1", provider_folder_id: "INBOX",
      from_address: "client@acme.test", to_address: "mailbox@example.com",
      subject: "Quarterly report", received_at: Time.current, read: false, has_attachment: false
    )
  end

  it "lazily creates the discussion and posts a Scout (AI) message" do
    result = described_class.announce(email_message: message) { "Hello **[event](/calendar_events/9)**" }

    expect(result).not_to be_nil
    agent_thread = thread.reload.agent_thread
    expect(agent_thread).not_to be_nil, "should have created the discussion thread"
    expect(agent_thread).to be_email_chat
    expect(agent_thread.user).to eq(user)
    expect(agent_thread.workspace).to eq(workspace)
    expect(agent_thread.agent_messages.count).to eq(1)
    expect(result).to be_from_ai
    expect(result.user).to eq(user)
    expect(result.content).to include("/calendar_events/9")
  end

  it "posts into an existing discussion without creating a second thread" do
    existing = thread.create_agent_thread!(title: thread.subject, purpose: :email_chat, user: user, workspace: workspace)
    existing.agent_messages.create!(content: "@scout what's this?", author_type: :user, user: user)

    expect {
      described_class.announce(email_message: message) { "noted" }
    }.to change { existing.reload.agent_messages.count }.by(1)
    expect(thread.reload.agent_thread).to eq(existing)
  end

  it "renders the body in the mailbox owner's locale" do
    user.update!(locale: "fr")

    result = described_class.announce(email_message: message) { I18n.locale.to_s }

    expect(result.content).to eq("fr")
  end

  it "does not create a discussion when create_if_missing: false and none exists" do
    result = described_class.announce(email_message: message, create_if_missing: false) { "noted" }

    expect(result).to be_nil
    expect(thread.reload.agent_thread).to be_nil
  end

  it "no-ops on a blank body without creating a thread" do
    result = described_class.announce(email_message: message) { "" }

    expect(result).to be_nil
    expect(thread.reload.agent_thread).to be_nil
  end

  it "no-ops when the mailbox has no owner" do
    no_owner_account = EmailAccount.create!(
      workspace: workspace, email_address: "shared@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    no_owner_account.email_account_users.create!(user: user, owner: false, can_read: true)
    orphan_thread = no_owner_account.email_threads.create!(subject: "Orphan")
    orphan_message = no_owner_account.email_messages.create!(
      email_thread: orphan_thread, provider_message_id: "m-2", provider_folder_id: "INBOX",
      from_address: "x@acme.test", to_address: "shared@example.com",
      subject: "Orphan", received_at: Time.current, read: false, has_attachment: false
    )

    expect(described_class.announce(email_message: orphan_message) { "noted" }).to be_nil
    expect(orphan_thread.reload.agent_thread).to be_nil
  end

  it "no-ops on a nil email message" do
    expect(described_class.announce(email_message: nil) { "noted" }).to be_nil
  end

  private

  def create_user(email)
    User.create!(
      workspace: workspace, email_address: email, name: email.split("@").first,
      password: "password123", password_confirmation: "password123"
    )
  end
end
