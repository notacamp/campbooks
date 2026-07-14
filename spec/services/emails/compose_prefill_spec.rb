require "rails_helper"

RSpec.describe Emails::ComposePrefill, type: :service do
  let(:account) { create(:email_account, email_address: "me@example.com") }

  def message_with(**attrs)
    create(:email_message, email_account: account, **attrs)
  end

  def prefill(message, mode)
    described_class.for(message: message, mode: mode)
  end

  describe "reply" do
    it "prefills To with the original sender" do
      message = message_with(from_address: "sender@example.com")

      expect(prefill(message, :reply).to).to eq("sender@example.com")
    end

    it "decodes an HTML-escaped stored sender (rows synced before the Zoho client decoded)" do
      message = message_with(from_address: "&quot;Supplier&quot; &lt;sender@example.com&gt;")

      expect(prefill(message, :reply).to).to eq('"Supplier" <sender@example.com>')
    end
  end

  describe "reply_all" do
    it "targets the sender plus the other recipients, minus the receiving account" do
      message = message_with(
        from_address: "sender@example.com",
        to_address: "me@example.com, other@example.com"
      )

      expect(prefill(message, :reply_all).to).to eq("sender@example.com, other@example.com")
    end

    it "drops the receiving account in Display Name <addr> form" do
      message = message_with(
        from_address: "sender@example.com",
        to_address: "Me Myself <me@example.com>, other@example.com"
      )

      expect(prefill(message, :reply_all).to).to eq("sender@example.com, other@example.com")
    end

    it "drops the receiving account when stored HTML-escaped (rows synced before the Zoho client decoded)" do
      message = message_with(
        from_address: "sender@example.com",
        to_address: "&lt;me@example.com&gt;,&lt;other@example.com&gt;"
      )

      expect(prefill(message, :reply_all).to).to eq("sender@example.com, <other@example.com>")
    end

    it "decodes escaped cc recipients and drops the receiving account from cc" do
      message = message_with(
        from_address: "sender@example.com",
        to_address: "&lt;me@example.com&gt;",
        cc_address: "&quot;Team&quot; &lt;team@example.com&gt;,&lt;me@example.com&gt;"
      )

      result = prefill(message, :reply_all)
      expect(result.to).to eq("sender@example.com")
      expect(result.cc).to eq('"Team" <team@example.com>')
    end
  end

  describe "subject" do
    it "decodes escaped entities before prefixing" do
      message = message_with(from_address: "sender@example.com", subject: "Quote &amp; delivery")

      expect(prefill(message, :reply).subject).to eq("Re: Quote & delivery")
    end
  end
end
