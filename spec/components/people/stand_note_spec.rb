# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::People::StandNote, type: :component do
  around { |example| travel_to(Time.zone.local(2026, 9, 3, 12, 0, 0)) { example.run } }

  let(:workspace) { create(:workspace) }
  let(:person) { create(:person, workspace: workspace, name: "Sofia Martins") }
  let(:org) { create(:organization, workspace: workspace, name: "Cloudhost") }

  def standing(verb: nil, detail: nil, detail_kind: nil, money: nil, needs_you: false,
               subject: "Q3 deck", wait_days: 3, feed_item_id: nil, email_message_id: nil)
    People::Standing::Result.new(
      verb: verb, detail: detail, detail_kind: detail_kind, money: money,
      needs_you: needs_you, thread_id: nil, overdue_days: wait_days.to_i,
      kind: needs_you ? :attention : :none, subject: subject,
      wait_days: wait_days.to_i, feed_item_id: feed_item_id,
      email_message_id: email_message_id
    )
  end

  def render_note(standing:, counterpart: person, **opts)
    ApplicationController.render(
      described_class.new(standing: standing, counterpart: counterpart, **opts),
      layout: false
    )
  end

  describe "reply verb" do
    let(:st) { standing(verb: :reply, detail: "the signed NDA", detail_kind: :ask_ai,
                        needs_you: true, feed_item_id: "f1", email_message_id: "m1") }
    let(:message) { instance_double("EmailMessage", id: "m1", received_at: 2.days.ago,
                                    ai_provenance: nil, respond_to?: true) }

    before { allow(message).to receive(:respond_to?).with(:ai_provenance).and_return(true) }

    it "shows the reply_ask sentence" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("signed NDA")
      expect(html).to include("where_things_stand").or include("where things stand")
    end

    it "shows the Draft reply chip when can_send and not draft_present" do
      html = render_note(standing: st, reply_target: message, can_send: true, draft_present: false)
      expect(html).to include("Draft reply")
    end

    it "hides the Draft reply chip when draft_present" do
      html = render_note(standing: st, reply_target: message, can_send: true, draft_present: true)
      expect(html).not_to include("Draft reply")
    end

    it "appends the draft_below sentence when draft_present and verb reply" do
      html = render_note(standing: st, reply_target: message, can_send: true, draft_present: true)
      expect(html).to include("below")
    end

    it "shows the Done chip when feed_item_id present" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Done")
    end

    it "shows the Snooze chip when email_message_id present" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Snooze")
    end
  end

  describe "nudge verb" do
    let(:st) { standing(verb: :nudge, detail: "Q3 report", detail_kind: :reason,
                        needs_you: true, feed_item_id: "f2") }
    let(:message) { instance_double("EmailMessage", id: "m2", received_at: 5.days.ago,
                                    ai_provenance: nil, respond_to?: false) }

    it "shows the nudge sentence" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Q3 deck")
    end

    it "shows Draft follow-up chip" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Draft follow-up")
    end

    it "shows Let it go chip when feed_item_id present" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Let it go")
    end
  end

  describe "decide verb with prompt detail" do
    let(:st) { standing(verb: :decide, detail: "Please approve the budget.", detail_kind: :prompt,
                        needs_you: true, feed_item_id: "f3") }

    it "carries the prompt in data-scout-ask-text-value" do
      html = render_note(standing: st, counterpart: person)
      expect(html).to include("scout-ask")
      expect(html).to include("Please approve the budget.")
    end

    it "shows Ask Scout chip" do
      html = render_note(standing: st, counterpart: person)
      expect(html).to include("Ask Scout")
    end

    it "shows Done chip when feed_item_id present" do
      html = render_note(standing: st, counterpart: person)
      expect(html).to include("Done")
    end
  end

  describe "decide verb with ask detail (no prompt)" do
    let(:st) { standing(verb: :decide, detail: "the contract", detail_kind: :ask_ai,
                        needs_you: true, feed_item_id: "f4", email_message_id: "m4") }
    let(:message) { instance_double("EmailMessage", id: "m4", received_at: 1.day.ago,
                                    ai_provenance: nil, respond_to?: false) }

    it "shows Draft reply chip (falls through to reply chips)" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Draft reply")
    end

    it "does not show Ask Scout chip" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).not_to include("Ask Scout")
    end
  end

  describe "pay / chase verbs" do
    let(:money) { { "amount_cents" => 120000, "currency" => "EUR", "due_date" => "2026-08-01", "days_late" => 33 } }
    let(:pay_st) { standing(verb: :pay, detail_kind: :money, money: money,
                             needs_you: true, feed_item_id: "f5") }
    let(:chase_st) { standing(verb: :chase, detail_kind: :money, money: money,
                               needs_you: true, feed_item_id: "f6") }

    it "shows Open in Money chip (pay)" do
      html = render_note(standing: pay_st, counterpart: org)
      expect(html).to include("Money")
    end

    it "shows Mark paid chip (pay, feed_item_id present)" do
      html = render_note(standing: pay_st, counterpart: org)
      expect(html).to include("Mark paid")
    end

    it "shows Open in Money chip (chase)" do
      html = render_note(standing: chase_st, counterpart: org)
      expect(html).to include("Money")
    end

    it "shows Mark paid chip (chase, feed_item_id present)" do
      html = render_note(standing: chase_st, counterpart: org)
      expect(html).to include("Mark paid")
    end
  end

  describe "no verb, ask detail" do
    let(:st) { standing(detail: "the signed NDA", detail_kind: :ask_quote) }
    let(:message) { instance_double("EmailMessage", id: "m7", received_at: 1.day.ago,
                                    ai_provenance: nil, respond_to?: false) }

    it "shows Draft reply chip when can_send" do
      html = render_note(standing: st, reply_target: message, can_send: true)
      expect(html).to include("Draft reply")
    end
  end

  describe "nothing to say" do
    let(:st) { People::Standing::Result.none }

    it "shows no_standing text and no chips" do
      html = render_note(standing: st, counterpart: person)
      expect(html).not_to include("Draft reply")
      expect(html).not_to include("Done")
    end
  end
end
