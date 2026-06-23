require "rails_helper"

RSpec.describe Feed::Sources::EmailAction do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  subject(:source) { described_class.new(user) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  # An actionable inbox candidate: Scout left a live action prompt.
  def todo_email(**attrs)
    create(:email_message, {
      email_account: account,
      ai_action_prompt: "Reply to the client",
      ai_todo_dismissed: false,
      skimmed_at: nil
    }.merge(attrs))
  end

  describe "inbox-folder gating" do
    context "when inbox folders resolve" do
      before { allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ]) }

      it "excludes archived (non-inbox) mail from candidates" do
        inbox = todo_email(provider_folder_id: "INBOX")
        archived = todo_email(provider_folder_id: "ARCHIVE")

        ids = source.candidates.map { |c| c[:subject].id }
        expect(ids).to include(inbox.id)
        expect(ids).not_to include(archived.id)
      end

      it "#still_valid? is true for inbox mail and false once it's archived" do
        inbox = todo_email(provider_folder_id: "INBOX")
        archived = todo_email(provider_folder_id: "ARCHIVE")

        expect(source.still_valid?(nil, inbox)).to be(true)
        expect(source.still_valid?(nil, archived)).to be(false)
      end
    end

    context "reply-state gating (conversations already answered)" do
      before { allow(Emails::InboxFolders).to receive(:ids_for).and_return([ "INBOX" ]) }

      it "excludes a thread the owner already answered (they hold the last word)" do
        answered = create(:email_thread, email_account: account, last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
        msg = todo_email(provider_folder_id: "INBOX", email_thread: answered)

        expect(source.candidates.map { |c| c[:subject].id }).not_to include(msg.id)
        expect(source.still_valid?(nil, msg)).to be(false)
      end

      it "keeps a thread still awaiting the owner's reply" do
        open_thread = create(:email_thread, email_account: account, last_outbound_at: nil, last_inbound_at: 1.hour.ago)
        msg = todo_email(provider_folder_id: "INBOX", email_thread: open_thread)

        expect(source.candidates.map { |c| c[:subject].id }).to include(msg.id)
        expect(source.still_valid?(nil, msg)).to be(true)
      end
    end

    context "when inbox folders can't be resolved (mail-client hiccup)" do
      before { allow(Emails::InboxFolders).to receive(:ids_for).and_return([]) }

      it "fails open — keeps the mail rather than emptying the feed" do
        inbox = todo_email(provider_folder_id: "INBOX")
        archived = todo_email(provider_folder_id: "ARCHIVE")

        ids = source.candidates.map { |c| c[:subject].id }
        expect(ids).to contain_exactly(inbox.id, archived.id)
        expect(source.still_valid?(nil, archived)).to be(true)
      end
    end
  end
end
