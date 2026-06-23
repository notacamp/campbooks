# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/skim_archive"

RSpec.describe Emails::SkimArchive do
  describe ".sanitize_ids" do
    it "coerces strings, drops blanks / zeros / negatives, and dedupes" do
      expect(described_class.sanitize_ids([ "3", "3", "x", "0", "-1", 5 ])).to eq([ 3, 5 ])
    end

    it "handles nil and empty input" do
      expect(described_class.sanitize_ids(nil)).to eq([])
      expect(described_class.sanitize_ids([])).to eq([])
    end
  end
end
