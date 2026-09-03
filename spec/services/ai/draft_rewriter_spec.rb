require "rails_helper"

RSpec.describe Ai::DraftRewriter, type: :service do
  subject(:rewriter) { described_class.new }

  let(:adapter) { instance_double("adapter") }
  let(:config) { { adapter: adapter, model: "test-model", temperature: 0.4 } }

  def stub_provider(reply)
    allow(Ai::Configuration).to receive(:for_any).and_return(config)
    allow(adapter).to receive(:chat).and_return(reply)
  end

  describe "#rewrite" do
    it "sends the tone instruction and the body, and returns the rewritten HTML" do
      stub_provider("<p>Short and sweet.</p>")

      result = rewriter.rewrite("<p>A rather long and meandering message.</p>", tone: "shorter")

      expect(result).to eq("<p>Short and sweet.</p>")
      expect(adapter).to have_received(:chat) do |args|
        expect(args[:system]).to include("shorter and more concise")
        expect(args[:messages].first[:content]).to include("A rather long and meandering message.")
        expect(args[:model]).to eq("test-model")
      end
    end

    it "includes the user's writing style in the prompt when given" do
      stub_provider("<p>Warmer.</p>")

      rewriter.rewrite("<p>Hi.</p>", tone: "warmer", style: "## How Sam writes\nUses first names.")

      expect(adapter).to have_received(:chat) do |args|
        expect(args[:system]).to include("How Sam writes")
      end
    end

    it "unwraps a fenced code block the model may add" do
      stub_provider("```html\n<p>Clean.</p>\n```")

      expect(rewriter.rewrite("<p>x</p>", tone: "firmer")).to eq("<p>Clean.</p>")
    end

    it "returns nil for an unknown tone" do
      allow(Ai::Configuration).to receive(:for_any)

      expect(rewriter.rewrite("<p>x</p>", tone: "spicy")).to be_nil
      expect(Ai::Configuration).not_to have_received(:for_any)
    end

    it "returns nil for a blank body" do
      expect(rewriter.rewrite("   ", tone: "shorter")).to be_nil
    end

    it "returns nil when no provider is configured" do
      allow(Ai::Configuration).to receive(:for_any).and_return(nil)
      allow(Ai::LegacyFallback).to receive(:allowed?).and_return(false)

      expect(rewriter.rewrite("<p>x</p>", tone: "shorter")).to be_nil
    end

    it "returns nil when the completion is blank" do
      stub_provider("   ")

      expect(rewriter.rewrite("<p>x</p>", tone: "shorter")).to be_nil
    end

    it "never raises — a provider error yields nil" do
      allow(Ai::Configuration).to receive(:for_any).and_return(config)
      allow(adapter).to receive(:chat).and_raise(StandardError, "boom")

      expect { @out = rewriter.rewrite("<p>x</p>", tone: "shorter") }.not_to raise_error
      expect(@out).to be_nil
    end
  end
end
