require "rails_helper"

RSpec.describe Reminders::EmailExtractionJob do
  let(:email) { create(:email_message) }

  it "skips extraction when the gate rejects the email" do
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(false)
    expect(Ai::ReminderExtractor).not_to receive(:new)
    expect(Reminders::Builder).not_to receive(:call)
    described_class.new.perform(email.id)
  end

  it "extracts and builds when the gate passes" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(true)
    allow_any_instance_of(Ai::ReminderExtractor).to receive(:extract).and_return([ { "reminder_type" => "deadline" } ])

    expect(Reminders::Builder).to receive(:call)
      .with(hash_including(source: email))
      .and_return([ instance_double(Reminder) ])

    described_class.new.perform(email.id)
  end

  it "skips extraction when no text AI provider is configured" do
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(true)
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)

    expect(Ai::ReminderExtractor).not_to receive(:new)
    expect(Reminders::Builder).not_to receive(:call)

    described_class.new.perform(email.id)
  end

  it "no-ops for a missing email" do
    expect { described_class.new.perform(-1) }.not_to raise_error
  end
end
