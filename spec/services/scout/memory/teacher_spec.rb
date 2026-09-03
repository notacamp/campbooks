require "rails_helper"

RSpec.describe Scout::Memory::Teacher do
  let(:ws) { Workspace.create!(name: "TE WS", slug: "te-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "te-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:teacher) { described_class.new(workspace: ws, user: user) }

  after { described_class.ai_mapper = nil }

  describe "deterministic parsers" do
    it "creates a stream (group rule) and returns its entry id" do
      result = nil
      expect { result = teacher.learn("treat GitHub notifications as a stream") }
        .to change(InboxGroupRule, :count).by(1)
      expect(result.status).to eq(:created)
      rule = InboxGroupRule.last
      expect(result.entry_id).to eq("group:#{rule.id}")
      expect(rule.rule_type).to eq("sender")
      expect(rule.value).to eq("github")
      expect(rule.group_name).to eq("GitHub notifications")
    end

    it "creates a filing rule with a folder" do
      result = nil
      expect { result = teacher.learn("file anything from @edp.pt under Utilities") }
        .to change(EmailRule, :count).by(1)
      rule = EmailRule.last
      expect(result.entry_id).to eq("rule:#{rule.id}")
      expect(rule.mail_folder.name).to eq("Utilities")
      expect(rule.criteria).to eq("from" => [ "@edp.pt" ])
      expect(ws.mail_folders.where("LOWER(name) = ?", "utilities")).to exist
    end

    it "creates a tag rule" do
      expect { teacher.learn("tag mail from stripe.com as Receipts") }.to change(EmailRule, :count).by(1)
      expect(ws.tags.where("LOWER(name) = ?", "receipts")).to exist
      expect(EmailRule.last.tags.map(&:name)).to include("Receipts")
    end

    it "stars a contact for priority" do
      result = teacher.learn("mail from sofia@x.com is priority")
      contact = ws.contacts.find_by(email: "sofia@x.com")
      expect(contact.starred_at).to be_present
      expect(result.entry_id).to eq("starred:#{contact.id}")
    end

    it "blocks a contact" do
      result = teacher.learn("block noreply@spam.com")
      contact = ws.contacts.find_by(email: "noreply@spam.com")
      expect(contact).to be_blocked
      expect(result.entry_id).to eq("blocked:#{contact.id}")
    end

    it "sets the default signature by name" do
      sig = user.signatures.create!(name: "Work", content: "<p>Best</p>", is_default: false)
      result = teacher.learn("sign replies with Work")
      expect(sig.reload.is_default).to be(true)
      expect(result.entry_id).to eq("signature:#{sig.id}")
    end

    it "refuses a signature name that does not exist" do
      result = teacher.learn("sign replies with Nonexistent")
      expect(result.status).to eq(:unknown)
    end
  end

  describe "unknown sentences" do
    it "returns the refusal copy and creates nothing" do
      result = nil
      expect { result = teacher.learn("make me a sandwich") }.not_to change(EmailRule, :count)
      expect(result.status).to eq(:unknown)
      expect(result.message).to eq(I18n.t("scout_memory.teach.unknown"))
    end

    it "treats blank input as unknown" do
      expect(teacher.learn("   ").status).to eq(:unknown)
    end
  end

  describe "AI fallback seam" do
    it "runs the injected mapper's intent through the same executor when no parser matches" do
      described_class.ai_mapper = ->(_text) { { kind: :block, contact: "ai@x.com" } }
      result = nil
      expect { result = teacher.learn("please quiet down that noisy sender") }
        .to change { ws.contacts.where(list_status: :blocked).count }.by(1)
      expect(result.status).to eq(:created)
    end

    it "validates the mapper output and refuses an unknown shape" do
      described_class.ai_mapper = ->(_text) { { kind: :launch_missiles } }
      result = nil
      expect { result = teacher.learn("do something arbitrary") }.not_to change(Contact, :count)
      expect(result.status).to eq(:unknown)
    end

    it "refuses a shape that is missing required keys" do
      described_class.ai_mapper = ->(_text) { { kind: :block } } # no contact
      expect(teacher.learn("blah").status).to eq(:unknown)
    end
  end
end
