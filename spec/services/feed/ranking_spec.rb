require "rails_helper"

RSpec.describe Feed::Ranking do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:now)       { Time.current }
  subject(:ranking) { described_class.new(user, now: now) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def message(**attrs)
    create(:email_message, { email_account: account, received_at: 1.hour.ago }.merge(attrs))
  end

  def candidate(subject, score: 80, sort_at: now, attention: false, data: {})
    { subject: subject, sort_at: sort_at, score: score, attention: attention, data: data }
  end

  def rank(kind, cand)
    ranking.apply!([ [ kind, cand ] ])
    cand
  end

  describe "recency decay" do
    it "keeps a fresh item at full strength" do
      c = rank("follow_up", candidate(message, score: 80, sort_at: 2.hours.ago))
      expect(c[:score]).to eq(80)
    end

    it "does not decay future-dated items (a reminder due next week keeps its proximity score)" do
      c = rank("reminder", candidate(message, score: 60, sort_at: 7.days.from_now))
      expect(c[:score]).to eq(60)
    end

    it "halves the score every half-life past the action moment" do
      c = rank("follow_up", candidate(message, score: 80, sort_at: (14.days + 36.hours).ago))
      expect(c[:score]).to be_within(2).of(40)
    end

    it "sinks a years-old follow-up to ~zero so it can't outrank fresh mail" do
      old = rank("follow_up", candidate(message, score: 80, attention: true, sort_at: 2.years.ago))
      fresh = rank("email_action", candidate(message, score: 45, sort_at: 1.hour.ago))

      expect(old[:score]).to be < 3
      expect(old[:score]).to be < fresh[:score]
    end
  end

  describe "attention gating" do
    it "keeps a fresh nomination in the attention cluster" do
      c = rank("follow_up", candidate(message, score: 80, attention: true, sort_at: 1.day.ago))
      expect(c[:attention]).to be(true)
    end

    it "demotes a nomination once decay drags it under the floor" do
      c = rank("follow_up", candidate(message, score: 80, attention: true, sort_at: 60.days.ago))
      expect(c[:attention]).to be(false)
    end

    it "never promotes an item the source didn't nominate" do
      c = rank("tag_suggestion", candidate(message, score: 100, attention: false, sort_at: now))
      expect(c[:attention]).to be(false)
    end
  end

  describe "relevance boosts" do
    it "lifts mail from a starred contact above an otherwise identical card" do
      starred = message(contact: create(:contact, workspace: workspace, starred_at: now))
      plain = message

      a = rank("email_action", candidate(starred, score: 45, sort_at: 1.hour.ago))
      b = rank("email_action", candidate(plain, score: 45, sort_at: 1.hour.ago))

      expect(a[:score]).to eq(b[:score] + described_class::STARRED_CONTACT_BOOST)
    end

    it "lifts mail from a contact with an analyzed relationship, but not unknown/self" do
      client = message(contact: create(:contact, workspace: workspace, relationship_type: "client"))
      unknown = message(contact: create(:contact, workspace: workspace, relationship_type: "unknown"))

      a = rank("email_action", candidate(client, score: 45, sort_at: 1.hour.ago))
      b = rank("email_action", candidate(unknown, score: 45, sort_at: 1.hour.ago))

      expect(a[:score]).to eq(b[:score] + described_class::KNOWN_RELATIONSHIP_BOOST)
    end

    it "sinks bulk-category mail and lifts important-category mail" do
      promo = rank("tag_suggestion", candidate(message(category: "promotions"), score: 15, sort_at: 1.hour.ago))
      important = rank("email_action", candidate(message(category: "important"), score: 45, sort_at: 1.hour.ago))
      plain = rank("email_action", candidate(message, score: 45, sort_at: 1.hour.ago))

      expect(promo[:score]).to eq(0) # 15 - 25, clamped
      expect(important[:score]).to eq(plain[:score] + described_class::IMPORTANT_CATEGORY_BOOST)
    end

    it "lifts a thread the user has written in over one they never touched" do
      engaged_thread = EmailThread.create!(subject: "Quote", email_account: account, last_outbound_at: 1.day.ago)
      cold_thread = EmailThread.create!(subject: "FYI", email_account: account)

      a = rank("email_action", candidate(message(email_thread: engaged_thread), score: 45, sort_at: 1.hour.ago))
      b = rank("email_action", candidate(message(email_thread: cold_thread), score: 45, sort_at: 1.hour.ago))

      expect(a[:score]).to eq(b[:score] + described_class::ENGAGED_THREAD_BOOST)
    end

    it "gives a small lift to a busy thread via the collapsed thread_count" do
      c = rank("email_action", candidate(message, score: 45, sort_at: 1.hour.ago, data: { "thread_count" => 5 }))
      plain = rank("email_action", candidate(message, score: 45, sort_at: 1.hour.ago))

      expect(c[:score]).to eq(plain[:score] + described_class::BUSY_THREAD_BOOST)
    end

    it "leaves non-email subjects to their intrinsic score" do
      reminder = create(:reminder, workspace: workspace)
      c = rank("reminder", candidate(reminder, score: 60, sort_at: 1.day.from_now))
      expect(c[:score]).to eq(60)
    end
  end

  describe "resilience" do
    it "falls back to the source score when a boost lookup blows up" do
      allow(Contact).to receive(:where).and_raise("boom")
      c = rank("email_action", candidate(message, score: 45, attention: true, sort_at: 1.hour.ago))

      expect(c[:score]).to eq(45)
      expect(c[:attention]).to be(true)
    end
  end
end
