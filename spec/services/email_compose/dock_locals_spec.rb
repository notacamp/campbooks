# frozen_string_literal: true

require "rails_helper"

RSpec.describe EmailCompose::DockLocals, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace:) }

  describe ".blank" do
    subject(:locals) { described_class.blank(user:) }

    it "returns a hash with all expected keys" do
      expected_keys = %i[
        mode message draft to cc bcc subject body quoted_body
        signatures signature_id accounts attachment_entries scout_draft
      ]
      expect(locals.keys).to match_array(expected_keys)
    end

    it "sets mode to :new_message" do
      expect(locals[:mode]).to eq(:new_message)
    end

    it "returns empty to/cc/bcc/body by default" do
      expect(locals[:to]).to eq("")
      expect(locals[:cc]).to eq("")
      expect(locals[:bcc]).to eq("")
      expect(locals[:body]).to eq("")
    end

    it "returns empty attachment_entries" do
      expect(locals[:attachment_entries]).to eq([])
    end

    context "with pre-filled to, subject, and body" do
      subject(:locals) do
        described_class.blank(
          user:,
          to:      "supplier@example.com",
          subject: "Missing invoice",
          body:    "Hello, please send the invoice."
        )
      end

      it "passes the values through" do
        expect(locals[:to]).to eq("supplier@example.com")
        expect(locals[:subject]).to eq("Missing invoice")
        expect(locals[:body]).to eq("Hello, please send the invoice.")
      end

      it "sets signature_id to nil when a body is prefilled" do
        expect(locals[:signature_id]).to be_nil
      end
    end

    context "without a pre-filled body" do
      it "populates accounts from the user's sendable accounts" do
        expect(locals[:accounts]).to be_an(Array)
      end

      it "populates signatures collection" do
        expect(locals[:signatures]).to respond_to(:each)
      end
    end
  end
end
