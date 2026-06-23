require "rails_helper"

RSpec.describe Emails::Threading do
  let(:account) { create(:email_account) }

  def msg(subject:, provider_thread_id: nil, from: "a@example.com")
    build(:email_message, email_account: account, email_scan_log: nil,
          subject: subject, from_address: from, provider_thread_id: provider_thread_id)
  end

  describe ".find_or_create with a provider thread id (Gmail/Graph)" do
    it "groups messages sharing the provider id, regardless of sender or subject prefix" do
      t1 = described_class.find_or_create(msg(subject: "Project", from: "ann@x.com", provider_thread_id: "T1"))
      t2 = described_class.find_or_create(msg(subject: "RE: Project", from: "bob@y.com", provider_thread_id: "T1"))
      expect(t2.id).to eq(t1.id)
      expect(account.email_threads.count).to eq(1)
    end

    it "keeps distinct conversations apart even when their subjects collide" do
      a = described_class.find_or_create(msg(subject: "Invoice", provider_thread_id: "A"))
      b = described_class.find_or_create(msg(subject: "Invoice", provider_thread_id: "B"))
      expect(b.id).not_to eq(a.id)
    end

    it "adopts a pre-existing subject-keyed thread at cutover and stamps the provider id" do
      legacy = described_class.find_or_create(msg(subject: "Quote"))
      expect(legacy.provider_thread_id).to be_nil

      adopted = described_class.find_or_create(msg(subject: "Re: Quote", provider_thread_id: "G9"))
      expect(adopted.id).to eq(legacy.id)
      expect(adopted.reload.provider_thread_id).to eq("G9")
    end
  end

  describe ".find_or_create without a provider thread id (Zoho/legacy)" do
    it "collapses reply/forward/case variants of the subject into one thread" do
      ids = [ "Pedido de simulação", "RE: FW: Pedido de simulação", "pedido de simulação" ]
              .map { |s| described_class.find_or_create(msg(subject: s)).id }
      expect(ids.uniq).to eq([ ids.first ])
    end

    it "always stamps subject_key (via the model safety net)" do
      thread = described_class.find_or_create(msg(subject: "FW: Hello"))
      expect(thread.subject_key).to eq("hello")
    end
  end

  describe ".find_or_create_outbound" do
    it "matches an existing inbound subject-keyed thread so a sent reply lands on it" do
      inbound  = described_class.find_or_create(msg(subject: "Customer query"))
      outbound = described_class.find_or_create_outbound(account, "Re: Customer query")
      expect(outbound.id).to eq(inbound.id)
    end
  end
end
