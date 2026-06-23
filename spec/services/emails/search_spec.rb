require "rails_helper"

RSpec.describe Emails::Search do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def run(folder_ids: nil, **params)
    described_class.new(user: user, params: params, folder_ids: folder_ids)
  end

  describe "#text_query?" do
    it "is true when a free-text query is present" do
      expect(run(q: "x").text_query?).to be(true)
      expect(run(unread: "1").text_query?).to be(false)
    end
  end

  describe "#scope (keyword / filter mode)" do
    it "is gated by accessible_to(user)" do
      mine = create(:email_message, email_account: account, subject: "shared hello")
      other = create(:email_account, workspace: workspace) # not shared with user
      theirs = create(:email_message, email_account: other, subject: "shared hello")

      result = run(q: "hello").scope
      expect(result).to include(mine)
      expect(result).not_to include(theirs)
    end

    it "matches q against subject, from_address and ai_summary (not body)" do
      by_subject = create(:email_message, email_account: account, subject: "Invoice 42", ai_summary: nil)
      by_summary = create(:email_message, email_account: account, subject: "Nope", ai_summary: "about an invoice")
      by_sender  = create(:email_message, email_account: account, subject: "Nope", from_address: "invoices@acme.com", ai_summary: nil)
      by_body    = create(:email_message, email_account: account, subject: "Nope", ai_summary: nil, from_address: "x@y.com", body: "the word invoice is in the body")

      result = run(q: "invoice").scope
      expect(result).to include(by_subject, by_summary, by_sender)
      expect(result).not_to include(by_body)
    end

    it "treats LIKE metacharacters literally" do
      literal = create(:email_message, email_account: account, subject: "100% off today")
      decoy = create(:email_message, email_account: account, subject: "1000 things")
      result = run(q: "100%").scope
      expect(result).to include(literal)
      expect(result).not_to include(decoy)
    end

    it "filters by sender" do
      hit = create(:email_message, email_account: account, from_address: "Jane <jane@acme.com>")
      miss = create(:email_message, email_account: account, from_address: "bob@other.com")
      result = run(sender: "acme").scope
      expect(result).to include(hit)
      expect(result).not_to include(miss)
    end

    it "filters by sender domain (leading @ optional)" do
      hit = create(:email_message, email_account: account, from_address: "Jane <jane@acme.com>")
      miss = create(:email_message, email_account: account, from_address: "jane@other.com")
      expect(run(domain: "acme.com").scope).to include(hit)
      expect(run(domain: "@acme.com").scope).to include(hit)
      expect(run(domain: "acme.com").scope).not_to include(miss)
    end

    it "filters unread only" do
      unread = create(:email_message, email_account: account, read: false)
      read = create(:email_message, email_account: account, read: true)
      result = run(unread: "1").scope
      expect(result).to include(unread)
      expect(result).not_to include(read)
    end

    it "filters by category" do
      important = create(:email_message, email_account: account, category: "important")
      promo = create(:email_message, email_account: account, category: "promotions")
      result = run(category: "important").scope
      expect(result).to include(important)
      expect(result).not_to include(promo)
    end

    it "filters by priority" do
      high = create(:email_message, email_account: account, ai_priority: :high)
      low = create(:email_message, email_account: account, ai_priority: :low)
      result = run(priority: "high").scope
      expect(result).to include(high)
      expect(result).not_to include(low)
    end

    it "filters by attachment presence" do
      with = create(:email_message, email_account: account, has_attachment: true)
      without = create(:email_message, email_account: account, has_attachment: false)
      result = run(has_attachment: "1").scope
      expect(result).to include(with)
      expect(result).not_to include(without)
    end

    it "filters by date range (inclusive of the whole day)" do
      old = create(:email_message, email_account: account, received_at: 10.days.ago)
      recent = create(:email_message, email_account: account, received_at: 1.day.ago)
      result = run(date_from: 3.days.ago.to_date.to_s).scope
      expect(result).to include(recent)
      expect(result).not_to include(old)
    end

    it "filters by resolved folder ids" do
      in_f1 = create(:email_message, email_account: account, provider_folder_id: "F1")
      in_f2 = create(:email_message, email_account: account, provider_folder_id: "F2")
      result = run(folder_ids: [ "F1" ]).scope
      expect(result).to include(in_f1)
      expect(result).not_to include(in_f2)
    end

    it "returns nothing when the chosen folder maps to no ids" do
      create(:email_message, email_account: account)
      expect(run(folder_ids: []).scope).to be_empty
    end

    it "filters by account_ids, intersected with readable accounts" do
      mine = create(:email_message, email_account: account)
      expect(run(account_ids: [ account.id.to_s ]).scope).to include(mine)

      unreadable = create(:email_account, workspace: workspace)
      expect(run(account_ids: [ unreadable.id.to_s ]).scope).to be_empty
    end

    describe "tag filters" do
      let(:tag_a) { Tag.create!(workspace: workspace, name: "Alpha", color: "#111111", source: :local) }
      let(:tag_b) { Tag.create!(workspace: workspace, name: "Beta", color: "#222222", source: :local) }

      it "any: matches a message carrying any selected tag" do
        m_a = create(:email_message, email_account: account).tap { |m| m.tags << tag_a }
        m_b = create(:email_message, email_account: account).tap { |m| m.tags << tag_b }
        untagged = create(:email_message, email_account: account)

        result = run(tag_ids: [ tag_a.id, tag_b.id ], tag_match: "any").scope
        expect(result).to include(m_a, m_b)
        expect(result).not_to include(untagged)
      end

      it "all: requires every selected tag" do
        both = create(:email_message, email_account: account).tap { |m| m.tags << tag_a << tag_b }
        only_a = create(:email_message, email_account: account).tap { |m| m.tags << tag_a }

        result = run(tag_ids: [ tag_a.id, tag_b.id ], tag_match: "all").scope
        expect(result).to include(both)
        expect(result).not_to include(only_a)
      end
    end

    it "orders by received_at descending" do
      older = create(:email_message, email_account: account, received_at: 5.days.ago)
      newer = create(:email_message, email_account: account, received_at: 1.hour.ago)
      expect(run.scope.to_a).to eq([ newer, older ])
    end
  end

  describe "#results (relevance search)" do
    let!(:message) do
      create(:email_message, email_account: account, subject: "quarterly numbers",
        from_address: "reports@acme.com", ai_summary: nil, read: false)
    end
    let!(:record) do
      SearchRecord.create!(workspace: workspace, searchable: message,
        filter_data: { "email_account_id" => account.id }, source_created_at: message.received_at)
    end

    def result_for(msg, sr)
      SearchService::Result.new(
        searchable_type: "EmailMessage", searchable_id: msg.id, search_record: sr,
        score: 0.9, title_similarity: 0.8, content_similarity: 0.9, recency_score: 1.0, matched_tags: []
      )
    end

    it "returns the ranked accessible messages from SearchService" do
      allow(SearchService).to receive(:search).and_return([ result_for(message, record) ])
      expect(run(q: "earnings").results).to eq([ message ])
    end

    it "drops semantic hits on accounts the user cannot read" do
      hidden_account = create(:email_account, workspace: workspace)
      hidden = create(:email_message, email_account: hidden_account)
      hidden_record = SearchRecord.create!(workspace: workspace, searchable: hidden,
        filter_data: { "email_account_id" => hidden_account.id }, source_created_at: hidden.received_at)
      allow(SearchService).to receive(:search).and_return([ result_for(hidden, hidden_record) ])

      expect(run(q: "x").results).to be_empty
    end

    it "still enforces SQL filters after the semantic narrowing" do
      allow(SearchService).to receive(:search).and_return([ result_for(message, record) ])
      expect(run(q: "x", unread: "1").results).to eq([ message ])

      message.update!(read: true)
      expect(run(q: "x", unread: "1").results).to be_empty
    end

    it "falls back to keyword results when the index is empty" do
      record.destroy
      expect(run(q: "quarterly").results).to include(message)
    end

    it "falls back to keyword results when SearchService raises" do
      allow(SearchService).to receive(:search).and_raise(StandardError, "boom")
      expect(run(q: "quarterly").results).to include(message)
    end
  end
end
