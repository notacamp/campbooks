# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Memory::Sources::Replies do
  let(:ws) { Workspace.create!(name: "Mem WS", slug: "mem-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "t-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  describe "#entries" do
    context "with a default signature" do
      let!(:sig) do
        user.signatures.create!(name: "Guilherme", content: "<p>--</p>", is_default: true)
      end

      it "produces a signature entry" do
        entry = source.entries.find { |e| e.id == "signature:#{sig.id}" }
        expect(entry).not_to be_nil
      end

      it "has the correct plain sentence" do
        entry = source.entries.find { |e| e.id == "signature:#{sig.id}" }
        expect(entry.plain).to eq("Sign replies with Guilherme.")
      end

      it "has :replies facet" do
        entry = source.entries.find { |e| e.id == "signature:#{sig.id}" }
        expect(entry.facet).to eq(:replies)
      end

      it "has :taught origin" do
        entry = source.entries.find { |e| e.id == "signature:#{sig.id}" }
        expect(entry.origin).to eq(:taught)
      end

      it "has only edit action" do
        entry = source.entries.find { |e| e.id == "signature:#{sig.id}" }
        expect(entry.actions).to eq(%i[edit])
      end
    end

    context "with a stated writing style" do
      before { user.update!(writing_style: "in a warm, concise voice. Always sign off friendly.") }

      it "produces a writing:stated entry" do
        entry = source.entries.find { |e| e.id == "writing:stated" }
        expect(entry).not_to be_nil
      end

      it "has the correct plain sentence (first sentence only)" do
        entry = source.entries.find { |e| e.id == "writing:stated" }
        # first_sentence keeps the trailing period; template appends another "."
        expect(entry.plain).to eq("Write replies in a warm, concise voice.")
      end

      it "has :taught origin" do
        entry = source.entries.find { |e| e.id == "writing:stated" }
        expect(entry.origin).to eq(:taught)
      end
    end

    context "with a learned writing style" do
      before { user.update!(writing_style_learned: "You write short, direct sentences. You avoid jargon.") }

      it "produces a writing:learned entry" do
        entry = source.entries.find { |e| e.id == "writing:learned" }
        expect(entry).not_to be_nil
      end

      it "has the correct plain sentence (first sentence only)" do
        entry = source.entries.find { |e| e.id == "writing:learned" }
        # first_sentence keeps the trailing period; template appends another "."
        expect(entry.plain).to eq("I write like you: You write short, direct sentences.")
      end

      it "has :learned origin" do
        entry = source.entries.find { |e| e.id == "writing:learned" }
        expect(entry.origin).to eq(:learned)
      end

      it "has confirm and remove actions" do
        entry = source.entries.find { |e| e.id == "writing:learned" }
        expect(entry.actions).to eq(%i[confirm remove])
      end
    end
  end

  describe "#remove" do
    context "for writing:learned entry" do
      before { user.update!(writing_style_learned: "You write short, direct sentences. You avoid jargon.") }

      it "clears writing_style_learned" do
        entry = source.entries.find { |e| e.id == "writing:learned" }
        source.remove(entry)
        expect(user.reload.writing_style_learned).to be_nil
      end
    end
  end

  describe "#confirm" do
    context "for writing:learned entry" do
      before { user.update!(writing_style_learned: "You write short, direct sentences. You avoid jargon.") }

      it "returns true and does not raise" do
        entry = source.entries.find { |e| e.id == "writing:learned" }
        expect { source.confirm(entry) }.not_to raise_error
        expect(source.confirm(entry)).to be_truthy
      end
    end
  end
end
