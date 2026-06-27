# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/skim_archive"

RSpec.describe Emails::SkimArchive do
  describe ".sanitize_ids" do
    it "keeps only well-formed uuids, dedupes, and drops malformed values" do
      u1 = "11111111-1111-4111-8111-111111111111"
      u2 = "22222222-2222-4222-8222-222222222222"
      expect(described_class.sanitize_ids([ u1, u1, "x", "0", "", u2 ])).to eq([ u1, u2 ])
    end

    it "handles nil and empty input" do
      expect(described_class.sanitize_ids(nil)).to eq([])
      expect(described_class.sanitize_ids([])).to eq([])
    end
  end
end
