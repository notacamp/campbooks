require "rails_helper"

RSpec.describe "Scout::Memory::Parsers" do
  describe Scout::Memory::Parsers::Stream do
    it "parses 'treat X as a stream'" do
      expect(described_class.call("treat GitHub notifications as a stream"))
        .to eq(kind: :stream, value: "github", group: "GitHub notifications")
    end

    it "parses an @domain into the sender value" do
      expect(described_class.call("make @github.com a stream"))
        .to eq(kind: :stream, value: "@github.com", group: "@github.com")
    end

    it "returns nil for unrelated text" do
      expect(described_class.call("hello there")).to be_nil
    end
  end

  describe Scout::Memory::Parsers::FileRule do
    it "parses 'file anything from X under Y'" do
      expect(described_class.call("file anything from @edp.pt under Utilities"))
        .to eq(kind: :file, from: "@edp.pt", folder: "Utilities")
    end

    it "tolerates quotes and a trailing period" do
      expect(described_class.call('file mail from "billing@acme.com" into Bills.'))
        .to eq(kind: :file, from: "billing@acme.com", folder: "Bills")
    end

    it "returns nil without a folder" do
      expect(described_class.call("file anything from @edp.pt")).to be_nil
    end
  end

  describe Scout::Memory::Parsers::TagRule do
    it "parses 'tag mail from X as Y' and strips a leading #" do
      expect(described_class.call("tag mail from stripe.com as #Receipts"))
        .to eq(kind: :tag, from: "stripe.com", tag: "Receipts")
    end
  end

  describe Scout::Memory::Parsers::Priority do
    it "parses 'mail from X is priority'" do
      expect(described_class.call("mail from sofia@x.com is priority"))
        .to eq(kind: :priority, contact: "sofia@x.com")
    end

    it "parses 'make X priority'" do
      expect(described_class.call("make sofia@x.com priority"))
        .to eq(kind: :priority, contact: "sofia@x.com")
    end
  end

  describe Scout::Memory::Parsers::Block do
    it "parses 'block X'" do
      expect(described_class.call("block noreply@spam.com"))
        .to eq(kind: :block, contact: "noreply@spam.com")
    end

    it "parses 'never show X in the stack'" do
      expect(described_class.call("never show noreply@spam.com in the stack"))
        .to eq(kind: :block, contact: "noreply@spam.com")
    end
  end

  describe Scout::Memory::Parsers::Signature do
    it "parses 'sign replies with X'" do
      expect(described_class.call("sign replies with Work")).to eq(kind: :signature, name: "Work")
    end
  end

  describe Scout::Memory::Parsers::Shared do
    it "detects emails" do
      expect(described_class.email?("a@b.com")).to be(true)
      expect(described_class.email?("just a name")).to be(false)
    end
  end
end
