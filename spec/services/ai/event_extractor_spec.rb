require "rails_helper"

RSpec.describe Ai::EventExtractor do
  it "anchors the guessed start to the email's received date, not the current date" do
    received = 1.year.ago
    email = create(:email_message, received_at: received, subject: "Project sync",
                                   body: "Let's meet at 3pm to review.")

    result = described_class.new(email).extract

    # The day after the email arrived — in the email's real year, not tomorrow-from-now.
    expect(result.start_at.to_date).to eq(received.in_time_zone.to_date + 1)
    expect(result.start_at.year).to eq(received.in_time_zone.year)
    expect(result.start_at.hour).to eq(15) # "3pm" parsed from the body
  end

  it "pins to the email's own day when the body says 'today'" do
    received = 200.days.ago
    email = create(:email_message, received_at: received, subject: "Standup",
                                   body: "Quick call today at 10am.")

    result = described_class.new(email).extract

    expect(result.start_at.to_date).to eq(received.in_time_zone.to_date)
    expect(result.start_at.hour).to eq(10)
  end

  it "falls back to the current date when the email has no received_at" do
    email = create(:email_message, received_at: nil, subject: "Untimed",
                                   body: "Let's sync at 2pm.")

    result = described_class.new(email).extract

    expect(result.start_at.to_date).to eq(Date.current + 1)
  end
end
