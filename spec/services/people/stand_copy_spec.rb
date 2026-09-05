# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::StandCopy do
  around { |e| I18n.with_locale(:en) { e.run } }

  def result(detail_kind:, detail: nil, money: nil, verb: nil, subject: "Q3 deck", wait_days: 2)
    People::Standing::Result.new(
      detail_kind: detail_kind,
      detail: detail,
      money: money,
      verb: verb,
      subject: subject,
      wait_days: wait_days,
      needs_you: verb.present?,
      thread_id: nil,
      overdue_days: 0,
      kind: verb ? :attention : :last_exchange,
      feed_item_id: nil,
      email_message_id: nil
    )
  end

  # ── line ──────────────────────────────────────────────────────────────────

  describe ".line" do
    it "ask_ai → Asks for <detail>" do
      r = result(detail_kind: :ask_ai, detail: "the signed NDA")
      expect(described_class.line(r)).to eq("Asks for the signed NDA")
    end

    it "ask_quote → Asks: “<quote>”" do
      r = result(detail_kind: :ask_quote, detail: "Can you send the NDA?")
      expect(described_class.line(r)).to eq("Asks: “Can you send the NDA?”")
    end

    it "reason → Waiting for <what>" do
      r = result(detail_kind: :reason, detail: "your decision")
      expect(described_class.line(r)).to eq("Waiting for your decision")
    end

    it "silence → No answer since <date> (from an ISO timestamp)" do
      r = result(detail_kind: :silence, detail: "2026-09-01T10:00:00Z")
      expect(described_class.line(r)).to eq("No answer since Sep 1")
    end

    it "prompt → sentence-cased prompt text, the rest untouched" do
      r = result(detail_kind: :prompt, detail: "please approve the Q3 budget")
      expect(described_class.line(r)).to eq("Please approve the Q3 budget")
    end

    it "you_wrote_last → You wrote last, <date>" do
      r = result(detail_kind: :you_wrote_last, detail: "2026-09-03")
      expect(described_class.line(r)).to eq("You wrote last, Sep 3")
    end

    it "money with pay verb → amount + due date + days overdue" do
      r = result(detail_kind: :money, verb: :pay,
                 money: { "amount_cents" => 50_000, "currency" => "EUR",
                          "due_date" => "2026-08-25", "days_late" => 10 })
      expect(described_class.line(r)).to eq("€500.00 due Aug 25 · 10 days late")
    end

    it "money with chase verb → you are owed <amount>" do
      r = result(detail_kind: :money, verb: :chase,
                 money: { "amount_cents" => 25_000, "currency" => "EUR",
                          "due_date" => "2026-08-20", "days_late" => 15 })
      expect(described_class.line(r)).to include("€250.00")
    end

    it "nil detail_kind → nil" do
      r = result(detail_kind: nil)
      expect(described_class.line(r)).to be_nil
    end
  end

  # ── note ──────────────────────────────────────────────────────────────────

  describe ".note" do
    it "verb reply + ask_ai → reply_ask template" do
      r = result(detail_kind: :ask_ai, verb: :reply, detail: "the signed NDA",
                 subject: "Q3 deck", wait_days: 3)
      expect(described_class.note(r, name: "Sofia"))
        .to eq("Sofia asked for the signed NDA 3 days ago in “Q3 deck” and is still waiting on you.")
    end

    it "verb reply uses the one / zero forms of the day count" do
      one  = result(detail_kind: :ask_ai, verb: :reply, detail: "the signed NDA", subject: "Q3 deck", wait_days: 1)
      zero = result(detail_kind: :ask_ai, verb: :reply, detail: "the signed NDA", subject: "Q3 deck", wait_days: 0)
      expect(described_class.note(one, name: "Sofia")).to include("1 day ago")
      expect(described_class.note(zero, name: "Sofia")).to include("today in “Q3 deck”")
    end

    it "verb reply + ask_quote → reply_quote template" do
      r = result(detail_kind: :ask_quote, verb: :reply, detail: "Can you send the NDA?",
                 subject: "Intro", wait_days: 1)
      note = described_class.note(r, name: "Rui")
      expect(note).to include("Can you send the NDA?")
    end

    it "verb reply + no detail → reply_plain template" do
      r = result(detail_kind: nil, verb: :reply, subject: "Proposal", wait_days: 5)
      note = described_class.note(r, name: "Ana")
      expect(note).to include("Ana")
      expect(note).to include("Proposal")
    end

    it "verb nudge → nudge template, without the thread's Re: prefix" do
      r = result(detail_kind: nil, verb: :nudge, subject: "Re: Re: Budget", wait_days: 6)
      expect(described_class.note(r, name: "Miguel"))
        .to eq("You wrote to Miguel 6 days ago about “Budget” and have not heard back.")
    end

    it "verb nudge + reason → appends nudge_reason" do
      r = result(detail_kind: :reason, verb: :nudge, detail: "your decision",
                 subject: "Budget", wait_days: 6)
      note = described_class.note(r, name: "Miguel")
      expect(note).to include("your decision")
    end

    it "verb decide + prompt → decide template" do
      r = result(detail_kind: :prompt, verb: :decide, detail: "Please approve the budget.",
                 subject: "Contract", wait_days: 3)
      note = described_class.note(r, name: "Ines")
      expect(note).to include("Please approve the budget.")
    end

    it "verb pay → pay note with formatted amount" do
      r = result(detail_kind: :money, verb: :pay, wait_days: 10,
                 money: { "amount_cents" => 50_000, "currency" => "EUR",
                          "due_date" => "2026-08-25", "days_late" => 10,
                          "reference" => nil })
      note = described_class.note(r, name: "Fasthost")
      expect(note).to include("Fasthost")
      expect(note).to include("€500.00")
    end

    it "verb chase → chase note" do
      r = result(detail_kind: :money, verb: :chase, wait_days: 14,
                 money: { "amount_cents" => 25_000, "currency" => "EUR",
                          "due_date" => "2026-08-20", "days_late" => 14,
                          "reference" => "INV-42" })
      note = described_class.note(r, name: "ClientCo")
      expect(note).to include("ClientCo")
      expect(note).to include("INV-42")
    end

    it "no verb + you_wrote_last → you_wrote_last template" do
      r = result(detail_kind: :you_wrote_last, verb: nil, detail: "2026-09-01")
      note = described_class.note(r, name: "Sofia")
      expect(note).to be_present
    end

    it "no verb + ask_ai + date → fresh_ask template" do
      r = result(detail_kind: :ask_ai, verb: nil, detail: "the invoice", subject: "Budget")
      note = described_class.note(r, name: "Sofia", date: Date.new(2026, 9, 1))
      expect(note).to include("Sofia")
      expect(note).to include("invoice")
    end

    it "no verb + ask_quote + date → fresh_quote template" do
      r = result(detail_kind: :ask_quote, verb: nil, detail: "When can we meet?", subject: "Intro")
      note = described_class.note(r, name: "Rui", date: Date.new(2026, 9, 2))
      expect(note).to include("When can we meet?")
    end
  end
end
