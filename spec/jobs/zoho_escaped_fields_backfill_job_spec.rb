require "rails_helper"

RSpec.describe Emails::ZohoEscapedFieldsBackfillJob, type: :job do
  let(:zoho_account) { create(:email_account, provider: :zoho, email_address: "me@example.com") }

  it "decodes HTML-entity-escaped fields on Zoho-synced messages" do
    message = create(
      :email_message,
      email_account: zoho_account,
      from_address: "&quot;Supplier&quot; &lt;geral@supplier.example.com&gt;",
      to_address: "&lt;me@example.com&gt;,&lt;other@example.com&gt;",
      cc_address: "&lt;team@example.com&gt;",
      subject: "Quote &amp; delivery",
      summary: "Wood &gt; steel"
    )

    described_class.perform_now

    message.reload
    expect(message.from_address).to eq('"Supplier" <geral@supplier.example.com>')
    expect(message.to_address).to eq("<me@example.com>,<other@example.com>")
    expect(message.cc_address).to eq("<team@example.com>")
    expect(message.subject).to eq("Quote & delivery")
    expect(message.summary).to eq("Wood > steel")
  end

  it "leaves already-clean Zoho rows untouched" do
    message = create(
      :email_message,
      email_account: zoho_account,
      from_address: "sender@example.com",
      to_address: "me@example.com",
      subject: "Plain subject"
    )

    expect { described_class.perform_now }.not_to change { message.reload.attributes }
  end

  it "never touches non-Zoho accounts, where entities are legitimate literal text" do
    google_account = create(:email_account, provider: :google)
    message = create(
      :email_message,
      email_account: google_account,
      subject: "Escaping &lt;div&gt; tags in CSS"
    )

    expect { described_class.perform_now }.not_to change { message.reload.subject }
  end
end
