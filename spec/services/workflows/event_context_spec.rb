require "rails_helper"

RSpec.describe Workflows::EventContext, type: :service do
  let(:workspace) { create(:workspace) }

  it "exposes the event to Liquid as event.*" do
    event = create(:event, workspace: workspace, name: "invoice.flagged",
                           payload: { "amount" => 42 })
    ctx = described_class.new(event)

    liquid = ctx.liquid_context["event"]
    expect(liquid["name"]).to eq("invoice.flagged")
    expect(liquid["payload"]).to eq("amount" => 42)
    expect(liquid["occurred_at"]).to eq(event.occurred_at.iso8601)
  end

  it "records trigger_data and step_input referencing the event" do
    event = create(:event, workspace: workspace, name: "contact.starred")
    ctx = described_class.new(event)
    expect(ctx.trigger_data).to include("type" => "event", "event_id" => event.id, "name" => "contact.starred")
    expect(ctx.step_input).to eq("event_id" => event.id)
  end

  it "exposes subject and source_event for emit_event chaining" do
    contact = create(:contact, workspace: workspace)
    event = create(:event, workspace: workspace, subject: contact)
    ctx = described_class.new(event)
    expect(ctx.subject).to eq(contact)
    expect(ctx.source_event).to eq(event)
  end

  context "when the subject is an EmailMessage" do
    it "surfaces it as email_message and feeds its documents to conditions" do
      email = create(:email_message)
      event = create(:event, workspace: workspace, name: "email.received", subject: email)
      ctx = described_class.new(event)

      expect(ctx.email_message).to eq(email)
      expect(ctx.documents).to eq(email.documents.to_a)
    end
  end

  context "when the subject is a Document" do
    it "exposes the single document to conditions and no email_message" do
      document = create(:document, workspace: workspace)
      event = create(:event, workspace: workspace, name: "document.approved", subject: document)
      ctx = described_class.new(event)

      expect(ctx.email_message).to be_nil
      expect(ctx.documents).to eq([ document ])
    end
  end

  it "exposes an actor label drop" do
    user = create(:user, workspace: workspace, name: "Ada")
    event = create(:event, workspace: workspace, actor: user)
    actor = described_class.new(event).liquid_context["event"]["actor"]
    expect(actor).to include("type" => "User", "id" => user.id)
    expect(actor["label"]).to be_present
  end
end
