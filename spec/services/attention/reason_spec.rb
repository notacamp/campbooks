# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::Reason do
  describe "#positive?" do
    it "is true for the reasons that raise a counterpart" do
      %w[replies_fast replies two_way meetings invoices pays_promptly engaged
         starred allowed vip urgent_sender taught_important org_lead].each do |key|
        expect(described_class.new(key: key)).to be_positive, "expected #{key} to be positive"
      end
    end

    it "is false for the reasons that only ever lower a counterpart" do
      %w[ignored dismissed service taught_unimportant blocked new].each do |key|
        expect(described_class.new(key: key)).not_to be_positive, "expected #{key} not to be positive"
      end
    end
  end

  describe ".from_h round-trip" do
    it "rebuilds an equal Reason from #to_h (string keys)" do
      reason = described_class.new(key: "replies_fast", params: { hours: 3 })
      expect(described_class.from_h(reason.to_h)).to eq(reason)
    end

    it "accepts symbol keys too" do
      reason = described_class.from_h(key: "two_way", params: { count: 4 })
      expect(reason.key).to eq("two_way")
      expect(reason.params).to eq("count" => 4)
    end

    it "stringifies the key and param keys" do
      reason = described_class.new(key: :meetings, params: { count: 2 })
      expect(reason.key).to eq("meetings")
      expect(reason.to_h).to eq("key" => "meetings", "params" => { "count" => 2 })
    end
  end

  describe "#sentence" do
    it "interpolates the params into the localized reason" do
      reason = described_class.new(key: "replies_fast", params: { hours: 3 })
      expect(reason.sentence).to eq("You usually answer within 3 hours")
    end

    it "renders a param-free reason" do
      expect(described_class.new(key: "pays_promptly").sentence).to eq("You settle their bills on time")
    end

    it "raises MissingTranslationData for an unknown key" do
      expect { described_class.new(key: "not_a_real_reason").sentence }
        .to raise_error(I18n::MissingTranslationData)
    end
  end
end
