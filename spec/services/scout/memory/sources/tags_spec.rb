# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::Tags do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with a tag that has a prompt" do
      let!(:tag) do
        t = create(:tag, workspace: ws, name: "invoice")
        t.update!(prompt: "Invoices and receipts.")
        t
      end

      it "produces one entry" do
        expect(source.entries.size).to eq(1)
      end

      it "has the correct plain sentence" do
        expect(source.entries.first.plain).to eq("Tag mail as #invoice when it's about invoices and receipts.")
      end

      it "has :taught origin" do
        expect(source.entries.first.origin).to eq(:taught)
      end

      it "has the correct id" do
        expect(source.entries.first.id).to eq("tag:#{tag.id}")
      end

      it "has only edit action" do
        expect(source.entries.first.actions).to eq(%i[edit])
      end
    end

    context "with a prompt phrased as a test on the email" do
      let!(:tag) do
        create(:tag, workspace: ws, name: "accounting").tap do |t|
          t.update!(prompt: "The email contains content or attachments related to accounting, bookkeeping or tax. Second sentence.")
        end
      end

      it "reads as a clause about the mail, not a spliced prompt" do
        expect(source.entries.first.plain)
          .to eq("Tag mail as #accounting when it contains content or attachments related to accounting, bookkeeping or tax.")
      end
    end

    context "with a tag that has no prompt and is not hidden" do
      let!(:tag) { create(:tag, workspace: ws, name: "visible-noprompt") }

      it "produces no entries" do
        expect(source.entries).to be_empty
      end
    end

    context "with a hidden tag that was AI-classified (learned)" do
      let!(:tag) do
        t = create(:tag, workspace: ws, name: "Promotions", hidden: true)
        t.update_columns(classified_at: Time.current)
        t
      end

      it "produces one entry" do
        expect(source.entries.size).to eq(1)
      end

      it "has the correct plain sentence" do
        expect(source.entries.first.plain).to eq("Provider label Promotions stays hidden.")
      end

      it "has :learned origin" do
        expect(source.entries.first.origin).to eq(:learned)
      end

      it "has only confirm action" do
        expect(source.entries.first.actions).to eq(%i[confirm])
      end
    end

    context "with a hidden tag without classified_at (user-hidden)" do
      let!(:tag) do
        create(:tag, workspace: ws, name: "Hidden Manual", hidden: true)
      end

      it "has :taught origin" do
        expect(source.entries.first.origin).to eq(:taught)
      end

      it "has only edit action" do
        expect(source.entries.first.actions).to eq(%i[edit])
      end
    end
  end

  describe "#confirm" do
    let!(:tag) do
      t = create(:tag, workspace: ws, name: "Promo", hidden: true)
      t.update_columns(classified_at: Time.current)
      t
    end

    it "sets classification_reason to 'confirmed' and returns true" do
      entry = source.entries.first
      result = source.confirm(entry)
      expect(result).to be_truthy
      expect(tag.reload.classification_reason).to eq("confirmed")
    end
  end
end
