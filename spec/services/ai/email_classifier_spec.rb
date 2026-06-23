require "rails_helper"

RSpec.describe Ai::EmailClassifier, type: :service do
  subject(:classifier) { described_class.new(EmailMessage.new) }

  # Regression: Claude (and other models) wrap JSON in a ```json … ``` markdown
  # fence. The classifier used to call JSON.parse on the raw text, so every
  # response failed with "Invalid JSON", the error was swallowed, and the email
  # was left untagged. parse_json must strip the fence before parsing.
  describe "#parse_json" do
    def parse(text) = classifier.send(:parse_json, text)

    it "parses a ```json fenced object (the observed Claude format)" do
      expect(parse(%(```json\n{"tags": ["notifications"]}\n```)))
        .to eq("tags" => [ "notifications" ])
    end

    it "parses a bare ``` fenced object" do
      expect(parse(%(```\n{"tags": ["software"]}\n```)))
        .to eq("tags" => [ "software" ])
    end

    it "parses plain (unfenced) JSON unchanged" do
      expect(parse(%({"flagged": false})))
        .to eq("flagged" => false)
    end

    it "parses a fenced JSON array" do
      expect(parse(%(```json\n["a", "b"]\n```)))
        .to eq([ "a", "b" ])
    end

    it "tolerates surrounding whitespace and an uppercase language hint" do
      expect(parse(%(\n  ```JSON\n{"tags": []}\n```  \n)))
        .to eq("tags" => [])
    end

    it "still raises on genuinely malformed JSON" do
      expect { parse("not json at all") }.to raise_error(JSON::ParserError)
    end
  end

  # Regression: tag names are not globally unique across workspaces. The classifier
  # used unscoped Tag.where / Tag.find_by, so once tagging actually ran it could offer
  # and attach another workspace's tag. Both lookups must be scoped to the email's own
  # workspace.
  describe "#classify! workspace scoping" do
    let(:ws_a) { create(:workspace) }
    let(:ws_b) { create(:workspace) }
    let(:account) { create(:email_account, workspace: ws_a) }
    let(:email) { create(:email_message, email_account: account) }
    let(:classifier) { described_class.new(email) }

    before do
      # ws_b's "promotional" created FIRST: an unscoped find_by would return it.
      ws_b.tags.create!(name: "promotional", color: "#000000", prompt: "promos for ws_b")
      ws_a.tags.create!(name: "promotional", color: "#111111", prompt: "promos for ws_a")
      allow(classifier).to receive(:pre_screen_flagged?).and_return(false)
      allow(classifier).to receive(:call_classify).and_return("tags" => [ "promotional" ])
    end

    it "assigns only the email's own workspace tag, never another workspace's" do
      classifier.classify!

      expect(email.reload.tags.pluck(:workspace_id)).to eq([ ws_a.id ])
    end

    it "offers only the email's own workspace tags to the model" do
      expect(classifier.send(:available_classification_tags).pluck(:workspace_id).uniq).to eq([ ws_a.id ])
    end
  end
end
