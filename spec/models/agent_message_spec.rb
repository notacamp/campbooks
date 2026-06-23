require "rails_helper"

RSpec.describe AgentMessage, type: :model do
  describe "#mentions_scout?" do
    def msg(content) = build(:agent_message, content: content)

    it "detects @scout case-insensitively, anywhere in the text" do
      expect(msg("hey @scout help").mentions_scout?).to be true
      expect(msg("@Scout summarize").mentions_scout?).to be true
      expect(msg("@SCOUT").mentions_scout?).to be true
    end

    it "is false without an @scout tag" do
      expect(msg("just a plain comment").mentions_scout?).to be false
      expect(msg("ask scout about it").mentions_scout?).to be false
    end

    it "ignores @scout embedded in an email address" do
      expect(msg("write to foo@scout.com").mentions_scout?).to be false
    end
  end
end
