require "rails_helper"

# The learning ConfidenceGuard + provenance that Reminders::Builder applies when a
# Learning::Memory is injected. Without a memory the builder behaves exactly as before
# (covered by builder_spec).
RSpec.describe "Reminders::Builder learning guard" do
  let(:workspace) { create(:workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }
  let(:sender)    { create(:email_message, from_address: "billing@acme.com", contact_id: contact.id) }

  def memory
    Learning::Memory.new(source: Learning::Sources::Reminders.new(workspace))
  end

  def item(overrides = {})
    { "reminder_type" => "payment_due", "title" => "Pay invoice",
      "due_date" => 10.days.from_now.to_date.iso8601, "all_day" => true, "confidence" => 0.9 }.merge(overrides)
  end

  def build(source: sender, mem: memory, items: [ item ])
    Reminders::Builder.call(workspace: workspace, source: source, raw_items: items, learning_memory: mem)
  end

  # Seed `n` acted-on reminders of `status` for the given sender identity.
  def seed(n, status:, type: :payment_due, contact_id: contact.id, domain: "acme.com")
    n.times do
      email = create(:email_message, from_address: "billing@#{domain}", contact_id: contact_id)
      create(:reminder, workspace: workspace, source: email, reminder_type: type, status: status)
    end
  end

  it "suppresses a new reminder when the sender's same-type reminders are consistently dismissed" do
    seed(3, status: :dismissed)

    expect { build }.not_to change(Reminder, :count)
  end

  it "stages the reminder when there is no learned signal" do
    expect { build }.to change(Reminder, :count).by(1)
  end

  it "does not suppress on a broad type-global dismissal from other senders" do
    3.times do |i|
      other = create(:contact, workspace: workspace)
      email = create(:email_message, from_address: "x@d#{i}.com", contact_id: other.id)
      create(:reminder, workspace: workspace, source: email, reminder_type: :payment_due, status: :dismissed)
    end

    expect { build }.to change(Reminder, :count).by(1)
  end

  it "records the learning provenance on a staged reminder (confirmed consensus)" do
    seed(3, status: :confirmed)

    # UUID PKs → Reminder.last is not chronological; use the builder's own return value.
    reminder = build.first
    prov = reminder.reload.extracted_data["_learning"]
    expect(prov).to include("label" => "confirmed", "signal" => "contact", "count" => 3, "total" => 3)
  end

  it "never suppresses when no memory is injected" do
    seed(3, status: :dismissed)

    expect {
      Reminders::Builder.call(workspace: workspace, source: sender, raw_items: [ item ])
    }.to change(Reminder, :count).by(1)
  end
end
