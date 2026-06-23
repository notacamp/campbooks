# frozen_string_literal: true

require "spec_helper"
require_relative "../../../app/services/emails/skim_archive"
require_relative "../../../app/services/emails/skim_restore"

RSpec.describe Emails::SkimRestore do
  describe "#call" do
    it "is a no-op (returns 0) when the id list is empty or sanitizes to nothing" do
      # Guards the safe path: a blank or forged-to-empty id list never touches mail.
      expect(Emails::SkimRestore.new(nil, nil).call).to eq(0)
      expect(Emails::SkimRestore.new(nil, []).call).to eq(0)
      expect(Emails::SkimRestore.new(nil, [ "0", "-1", "x" ]).call).to eq(0)
    end
  end
end
