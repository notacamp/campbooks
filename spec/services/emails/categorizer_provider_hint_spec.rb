require "rails_helper"

# The provider noise hint is rescue-only: it may pull the personal residual
# into a noise bucket, but never overrides a rule (machine sender, security
# subject, ...) and never surfaces a non-noise hint.
RSpec.describe Emails::Categorizer, "provider hint integration" do
  HintedEmail = Struct.new(:subject, :from_address, :provider_category_hint, keyword_init: true)
  PlainEmail = Struct.new(:subject, :from_address, keyword_init: true)

  it "rescues the personal residual when the provider says promotions" do
    email = HintedEmail.new(
      subject: "Our spring lookbook", from_address: "anna@brandstudio.example",
      provider_category_hint: :promotions
    )

    result = described_class.new(email).call

    expect(result.category).to eq(:promotions)
    expect(result.reasons).to eq([ "provider category label" ])
    expect(result.confidence).to be >= 0.9
  end

  it "rescues into social and updates the same way" do
    %i[social updates].each do |bucket|
      email = HintedEmail.new(
        subject: "Hello there", from_address: "someone@quietsender.example",
        provider_category_hint: bucket
      )

      expect(described_class.new(email).call.category).to eq(bucket)
    end
  end

  it "never overrides a machine-sender rule" do
    email = HintedEmail.new(
      subject: "Nightly build finished", from_address: "no-reply@ci.example",
      provider_category_hint: :updates
    )

    result = described_class.new(email).call

    expect(result.category).to eq(:notifications)
  end

  it "never overrides the security pre-screen path" do
    email = HintedEmail.new(
      subject: "Your verification code is 123456", from_address: "auto@login.example",
      provider_category_hint: :promotions
    )

    result = described_class.new(email).call

    expect(result.category).to eq(:important)
  end

  it "ignores non-noise hints" do
    email = HintedEmail.new(
      subject: "Quick question", from_address: "sam@humanmail.example",
      provider_category_hint: :personal
    )

    result = described_class.new(email).call

    expect(result.category).to eq(:personal)
    expect(result.confidence).to eq(0.4)
  end

  it "unchanged residual behavior when the email exposes no hint" do
    email = PlainEmail.new(subject: "Quick question", from_address: "sam@humanmail.example")

    result = described_class.new(email).call

    expect(result.category).to eq(:personal)
    expect(result.confidence).to eq(0.4)
  end
end
