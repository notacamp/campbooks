require "rails_helper"

# Per-source behaviour: which records each Feed::Source turns into candidates, and
# how still_valid? prunes a card whose record was handled.
RSpec.describe "Feed::Sources" do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace, email_address: "me@biz.example") }

  before { create(:email_account_user, user: user, email_account: account) }

  def keys(source) = source.candidates.map { |c| c[:dedupe_key] }

  describe Feed::Sources::EmailAction do
    subject(:source) { described_class.new(user) }

    it "includes Scout-flagged, unskimmed mail and excludes the rest" do
      flagged = create(:email_message, email_account: account, ai_action_prompt: "Reply to this", skimmed_at: nil)
      create(:email_message, email_account: account, ai_action_prompt: nil) # no prompt
      create(:email_message, email_account: account, ai_action_prompt: "x", skimmed_at: Time.current) # already skimmed
      other = create(:email_account, workspace: workspace)
      create(:email_message, email_account: other, ai_action_prompt: "hidden") # not readable

      expect(keys(source)).to contain_exactly("email_action:#{flagged.id}")
    end

    it "still_valid? drops a skimmed message" do
      m = create(:email_message, email_account: account, ai_action_prompt: "Reply", skimmed_at: Time.current)
      expect(source.still_valid?(double(data: {}), m)).to be(false)
    end
  end

  describe Feed::Sources::ReplyReminder do
    subject(:source) { described_class.new(user) }

    it "surfaces aged, reply-expected mail that has not been answered" do
      aged = create(:email_message, email_account: account, ai_action_prompt: "Needs a reply",
                    ai_suggested_actions: [ { "tool" => "draft_reply" } ], received_at: 5.days.ago)
      create(:email_message, email_account: account, ai_action_prompt: "Fresh",
             ai_suggested_actions: [ { "tool" => "draft_reply" } ], received_at: 1.hour.ago) # not aged

      expect(keys(source)).to contain_exactly("reply_reminder:#{aged.id}")
    end

    it "excludes a thread that has since been answered by the mailbox owner" do
      thread = EmailThread.create!(subject: "Question", email_account: account)
      create(:email_message, email_account: account, email_thread: thread, ai_action_prompt: "Reply",
             ai_suggested_actions: [ { "tool" => "draft_reply" } ], received_at: 5.days.ago,
             from_address: "client@example.com")
      create(:email_message, email_account: account, email_thread: thread, from_address: "me@biz.example",
             received_at: 2.days.ago) # the owner replied later

      expect(keys(source)).to be_empty
    end
  end

  describe Feed::Sources::TagSuggestion do
    subject(:source) { described_class.new(user) }

    it "surfaces filing-only suggestions and skips reply-worthy mail" do
      filing = create(:email_message, email_account: account,
                      ai_suggested_actions: [ { "tool" => "add_tag", "args" => { "tag_name" => "invoices" } } ])
      create(:email_message, email_account: account,
             ai_suggested_actions: [ { "tool" => "add_tag", "args" => { "tag_name" => "x" } },
                                     { "tool" => "draft_reply" } ]) # reply-worthy → excluded

      expect(keys(source)).to contain_exactly("tag_suggestion:#{filing.id}")
    end
  end
end
