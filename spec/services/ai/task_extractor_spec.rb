# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::TaskExtractor do
  it "re-raises transient provider errors so the job's retry_on gets its turn" do
    adapter = Object.new
    def adapter.chat(**) = raise Faraday::TooManyRequestsError, "429 from provider"

    with_config(adapter) do
      expect {
        described_class.new(source: nil, content: "please send the file", workspace: nil).extract
      }.to raise_error(Faraday::TooManyRequestsError)
    end
  end

  it "degrades to [] on non-transient failure" do
    adapter = Object.new
    def adapter.chat(**) = raise "malformed everything"

    with_config(adapter) do
      expect(
        described_class.new(source: nil, content: "please send the file", workspace: nil).extract
      ).to eq([])
    end
  end

  it "known tasks render as an exclusion block in the user message" do
    extractor = described_class.new(
      source: nil, content: "please send the file", workspace: nil,
      known_tasks: [ "Send the signed contract", "" ]
    )

    message = extractor.send(:user_message)

    expect(message).to include("<already_tracked_tasks>")
    expect(message).to include("- Send the signed contract")
  end

  it "no exclusion block when there are no known tasks" do
    extractor = described_class.new(source: nil, content: "please send the file", workspace: nil)

    expect(extractor.send(:user_message)).not_to include("<already_tracked_tasks>")
  end

  private

  # Swap Ai::Configuration.for_any for the duration of the block (mirrors the
  # singleton-swap pattern from the Minitest suite; converted to RSpec allow stub).
  def with_config(adapter)
    allow(Ai::Configuration).to receive(:for_any).and_return({ adapter: adapter, model: "m" })
    yield
  end
end
