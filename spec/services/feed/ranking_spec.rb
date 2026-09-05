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

    it "lifts mail from a sender whose mail runs urgent and sinks a low-urgency one" do
      hot = message(contact: create(:contact, workspace: workspace, communication_patterns: { "urgency_level" => "high" }))
      cool = message(contact: create(:contact, workspace: workspace, communication_patterns: { "urgency_level" => "low" }))
      plain = message

      a = rank("email_action", candidate(hot, score: 45, sort_at: 1.hour.ago))
      b = rank("email_action", candidate(cool, score: 45, sort_at: 1.hour.ago))
      p = rank("email_action", candidate(plain, score: 45, sort_at: 1.hour.ago))

      expect(a[:score]).to eq(p[:score] + described_class::SENDER_URGENCY_BOOST["high"])
      expect(b[:score]).to eq(p[:score] + described_class::SENDER_URGENCY_BOOST["low"])
    end
  end

  describe "seen-but-ignored demotion" do
    it "drifts a card down once it has been shown for days with no reaction" do
      c = candidate(message, score: 45, sort_at: 1.hour.ago)
      c[:seen_at] = 3.days.ago

      expect(rank("email_action", c)[:score]).to eq(45 - described_class::SEEN_IGNORE_PENALTY)
    end

    it "leaves a recently seen card at full strength" do
      c = candidate(message, score: 45, sort_at: 1.hour.ago)
      c[:seen_at] = 1.day.ago

      expect(rank("email_action", c)[:score]).to eq(45)
    end
  end

  describe "engagement multiplier" do
    def history_item(kind, **stamps)
      m = message
      FeedItem.create!({ user: user, workspace: workspace, kind: kind, subject: m,
                         dedupe_key: "#{kind}:#{m.id}", sort_at: now,
                         generated_at: now }.merge(stamps))
    end

    it "is exactly 1.0 with no history (cold start)" do
      expect(rank("email_action", candidate(message, score: 45, sort_at: 1.hour.ago))[:score]).to eq(45)
    end

    it "discounts a kind the user habitually dismisses" do
      12.times { history_item("tag_suggestion", dismissed_at: now) }

      c = rank("tag_suggestion", candidate(message, score: 40, sort_at: 1.hour.ago))

      # rate = 2/16 → multiplier 0.775
      expect(c[:score]).to eq((40 * 0.775).round)
    end

    it "rewards a kind the user habitually acts on" do
      12.times { history_item("reminder", acted_at: now) }
      reminder = create(:reminder, workspace: workspace)

      c = rank("reminder", candidate(reminder, score: 60, sort_at: 1.day.from_now))

      # rate = 14/16 → 1.225, clamped to the ceiling
      expect(c[:score]).to eq((60 * 1.2).round)
    end

    it "is blind to system expiries — churn is not disengagement" do
      12.times { history_item("calendar_event", expired_at: now) }

      c = rank("calendar_event", candidate(message, score: 60, sort_at: 1.hour.from_now))

      expect(c[:score]).to eq(60)
    end
  end

  describe "attention weight boosts (the user has attention rows)" do
    def attention_boost(weight)
      ((weight - described_class::ATTENTION_PIVOT) * described_class::ATTENTION_SPAN).round
    end

    # A contact whose person carries an AttentionWeight of `weight`.
    def weighted_contact(weight:, reasons: [], starred: false, confidence: 0.9)
      person = create(:person, workspace: workspace)
      contact = create(:contact, workspace: workspace, person: person,
                       starred_at: starred ? now : nil)
      AttentionWeight.create!(user: user, workspace: workspace, subject: person,
                              weight: weight, confidence: confidence, reasons: reasons,
                              computed_at: now)
      contact
    end

    it "boosts a high-weight sender by the centered weight, and never adds the legacy star boost" do
      # Also starred, to prove the fixed +STARRED_CONTACT_BOOST is not stacked on top.
      high  = message(contact: weighted_contact(weight: 0.9, starred: true))
      plain = message

      a = rank("email_action", candidate(high, score: 45, sort_at: 1.hour.ago))
      b = rank("email_action", candidate(plain, score: 45, sort_at: 1.hour.ago))

      expect(a[:score]).to eq(b[:score] + attention_boost(0.9))
      expect(a[:score]).to eq(b[:score] + 36)
    end

    it "penalizes a low-weight sender below the neutral prior" do
      low   = message(contact: weighted_contact(weight: 0.05))
      plain = message

      a = rank("email_action", candidate(low, score: 45, sort_at: 1.hour.ago))
      b = rank("email_action", candidate(plain, score: 45, sort_at: 1.hour.ago))

      expect(a[:score]).to eq(b[:score] + attention_boost(0.05))
    end

    it "gives a contact with no row the neutral prior (zero lift) even on a user who has rows" do
      weighted_contact(weight: 0.9) # ensures the user is not 'missing'
      bare = create(:contact, workspace: workspace, person: create(:person, workspace: workspace))

      c = rank("email_action", candidate(message(contact: bare), score: 45, sort_at: 1.hour.ago))
      expect(c[:score]).to eq(45 + attention_boost(described_class::ATTENTION_PIVOT))
      expect(c[:score]).to eq(45)
    end

    it "stamps the first positive reason and the rounded weight into data['why']/data['weight']" do
      contact = weighted_contact(weight: 0.9,
                                 reasons: [ { "key" => "replies_fast", "params" => { "hours" => 3 } } ])
      c = rank("email_action", candidate(message(contact: contact), score: 45, sort_at: 1.hour.ago))

      expect(c[:data]["why"]).to eq("key" => "replies_fast", "params" => { "hours" => 3 })
      expect(c[:data]["weight"]).to eq(0.9)
    end

    it "stamps the weight but no why when the row's reasons are all negative" do
      contact = weighted_contact(weight: 0.05,
                                 reasons: [ { "key" => "ignored", "params" => { "percent" => 80 } } ])
      c = rank("email_action", candidate(message(contact: contact), score: 45, sort_at: 1.hour.ago))

      expect(c[:data]).to have_key("weight")
      expect(c[:data]).not_to have_key("why")
    end

    it "drops a stale why when the row no longer has a positive reason" do
      contact = weighted_contact(weight: 0.05,
                                 reasons: [ { "key" => "ignored", "params" => { "percent" => 80 } } ])
      stale = { "key" => "replies_fast", "params" => { "hours" => 2 } }
      c = rank("email_action", candidate(message(contact: contact), score: 45, sort_at: 1.hour.ago,
                                         data: { "why" => stale }))

      expect(c[:data]).not_to have_key("why")
    end

    it "picks the first POSITIVE reason even when a negative one is listed first" do
      contact = weighted_contact(weight: 0.9, reasons: [
        { "key" => "ignored", "params" => { "percent" => 55 } },
        { "key" => "two_way", "params" => { "count" => 4 } }
      ])
      c = rank("email_action", candidate(message(contact: contact), score: 45, sort_at: 1.hour.ago))

      expect(c[:data]["why"]).to eq("key" => "two_way", "params" => { "count" => 4 })
    end

    it "lifts a late-payable Document via the vendor contact on its linked message" do
      contact = weighted_contact(weight: 0.9)
      msg = message(contact: contact)
      doc = create(:document, workspace: workspace)
      DocumentEmailMessage.create!(document: doc, email_message: msg)

      c = rank("late_payable", candidate(doc, score: 45, sort_at: 1.hour.ago))

      expect(c[:score]).to eq(45 + attention_boost(0.9))
      expect(c[:data]["weight"]).to eq(0.9)
    end

    it "leaves a reminder subject untouched (no contact, no stamping)" do
      weighted_contact(weight: 0.9) # user has rows
      reminder = create(:reminder, workspace: workspace)
      c = rank("reminder", candidate(reminder, score: 60, sort_at: 1.day.from_now))

      expect(c[:score]).to eq(60)
      expect(c[:data]).not_to have_key("weight")
      expect(c[:data]).not_to have_key("why")
    end
  end

  describe "no-decay for late bills" do
    it "does not decay late_payable even when sort_at is long past" do
      doc = create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
                   amount_cents: 10_000, due_date: 30.days.ago)
      c = rank("late_payable", candidate(doc, score: 90, sort_at: 30.days.ago, attention: true))
      expect(c[:score]).to eq(90)
    end

    it "does not decay late_receivable even when sort_at is long past" do
      doc = create(:document, :approved, :revenue_invoice, workspace: workspace,
                   amount_cents: 10_000, due_date: 30.days.ago)
      c = rank("late_receivable", candidate(doc, score: 90, sort_at: 30.days.ago, attention: true))
      expect(c[:score]).to eq(90)
    end

    it "still decays follow_up as before" do
      c = rank("follow_up", candidate(message, score: 80, sort_at: (14.days + 36.hours).ago))
      expect(c[:score]).to be_within(2).of(40)
    end
  end

  describe "resilience" do
    it "falls back to the source score when a boost lookup blows up" do
      allow(Contact).to receive(:where).and_raise("boom")
      c = rank("email_action", candidate(message, score: 45, attention: true, sort_at: 1.hour.ago))

      expect(c[:score]).to eq(45)
      expect(c[:attention]).to be(true)
    end

    it "leaves every candidate on its source score when the attention preload blows up" do
      allow(Attention::Weights).to receive(:new).and_raise("boom")
      c = rank("email_action", candidate(message, score: 45, attention: true, sort_at: 1.hour.ago))

      expect(c[:score]).to eq(45)
      expect(c[:attention]).to be(true)
    end
  end
end
