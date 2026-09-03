# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::EmailRules do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with a rule that archives mail into a folder" do
      let(:folder) { create(:mail_folder, workspace: ws, name: "Utilities") }
      let!(:rule) do
        ws.email_rules.create!(
          name: "EDP",
          criteria: { "from" => ["@edp.pt"] },
          archive: true,
          mail_folder: folder,
          created_by: user
        )
      end

      it "produces exactly one entry" do
        expect(source.entries.size).to eq(1)
      end

      it "has the correct plain sentence" do
        entry = source.entries.first
        expect(entry.plain).to eq("File anything from @edp.pt under Utilities and archive it.")
      end

      it "includes bold spans for the from value and folder name" do
        spans = source.entries.first.sentence.spans
        expect(spans).to include({ text: "@edp.pt", bold: true })
        expect(spans).to include({ text: "Utilities", bold: true })
      end

      it "has :filing facet" do
        expect(source.entries.first.facet).to eq(:filing)
      end

      it "has :taught origin" do
        expect(source.entries.first.origin).to eq(:taught)
      end

      it "has the correct id" do
        expect(source.entries.first.id).to eq("rule:#{rule.id}")
      end

      it "has edit and remove actions" do
        expect(source.entries.first.actions).to eq(%i[edit remove])
      end

      it "has the rule as record" do
        expect(source.entries.first.record).to eq(rule)
      end
    end

    context "with a rule that only applies a tag based on subject" do
      let!(:tag) { create(:tag, workspace: ws, name: "invoice") }
      let!(:rule) do
        ws.email_rules.create!(
          name: "Tag invoices",
          criteria: { "subject" => ["invoice"] },
          archive: false,
          created_by: user,
          tags: [tag]
        )
      end

      it "includes 'with \"invoice\" in the subject' in the plain text" do
        entry = source.entries.first
        expect(entry.plain).to include('with "invoice" in the subject')
      end

      it "includes 'tag it #invoice' in the plain text" do
        entry = source.entries.first
        expect(entry.plain).to include("tag it #invoice")
      end
    end
  end

  describe "#remove" do
    let!(:rule) do
      ws.email_rules.create!(
        name: "To remove",
        criteria: { "from" => ["@test.com"] },
        archive: true,
        created_by: user
      )
    end

    it "destroys the rule and returns true" do
      entry = source.entries.first
      expect { source.remove(entry) }.to change(EmailRule, :count).by(-1)
    end

    it "returns false for a rule belonging to a different workspace" do
      other_ws = Workspace.create!(name: "Other WS", slug: "other-#{SecureRandom.hex(4)}")
      other_user = other_ws.users.create!(name: "O", email_address: "o-#{SecureRandom.hex(4)}@example.com", password: "password123")
      other_rule = other_ws.email_rules.create!(
        name: "Foreign rule",
        criteria: { "from" => ["@foreign.com"] },
        archive: true,
        created_by: other_user
      )
      fake_entry = Scout::Memory::Entry.new(
        id: "rule:#{other_rule.id}",
        facet: :filing,
        sentence: Scout::Memory::Sentence.parse("something"),
        origin: :taught,
        source_key: :email_rules,
        record: other_rule,
        actions: %i[edit remove]
      )
      result = source.remove(fake_entry)
      expect(result).to be_falsey
      expect(EmailRule.where(id: other_rule.id)).to exist
    end
  end
end
