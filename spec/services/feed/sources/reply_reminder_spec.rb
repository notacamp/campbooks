require "rails_helper"

RSpec.describe Feed::Sources::ReplyReminder do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  subject(:source) { described_class.new(user) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ])
  end

  # Aged, high-priority mail that still expects a reply.
  def aged_email(**attrs)
    create(:email_message, {
      email_account: account,
      ai_action_prompt: "Please reply",
      ai_todo_dismissed: false,
      ai_priority: :high,
      received_at: 5.days.ago
    }.merge(attrs))
  end

  describe "#still_valid? — inbox gating" do
    it "drops an aged no-reply nudge once the mail is archived" do
      archived = aged_email(provider_folder_id: "ARCHIVE")
      item = double("item", data: { "reason" => "no_reply" })

      expect(source.still_valid?(item, archived)).to be(false)
    end

    it "keeps an aged no-reply nudge while the mail is in the inbox" do
      inbox = aged_email(provider_folder_id: "INBOX")
      item = double("item", data: { "reason" => "no_reply" })

      expect(source.still_valid?(item, inbox)).to be(true)
    end

    # The crucial regression guard: Tools::Snooze relocates the thread to a
    # Snoozed folder, so an inbox-only gate would suppress every snooze nudge.
    it "NEVER gates a due-snooze reminder — snoozed mail lives outside the inbox" do
      snoozed = aged_email(provider_folder_id: "SNOOZED")
      item = double("item", data: { "reason" => "snooze_due" })

      expect(source.still_valid?(item, snoozed)).to be(true)
    end
  end

  describe "#candidates — aged no-reply path" do
    it "excludes archived mail" do
      inbox = aged_email(provider_folder_id: "INBOX")
      archived = aged_email(provider_folder_id: "ARCHIVE")

      ids = source.candidates.map { |c| c[:subject].id }
      expect(ids).to include(inbox.id)
      expect(ids).not_to include(archived.id)
    end
  end
end
