# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tasks::ExtractionGate do
  let(:workspace) { Workspace.create!(name: "Gate WS") }
  let(:account) do
    EmailAccount.create!(
      workspace: workspace, email_address: "mailbox@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
  end

  def build_email(subject: "Hello", body: "", from: "sender@acme.test",
                  category: "personal", precedence: nil)
    account.email_messages.create!(
      provider_message_id: "m-#{SecureRandom.hex(4)}", provider_folder_id: "INBOX",
      from_address: from, to_address: "mailbox@example.com",
      subject: subject, body: body, category: category,
      header_precedence: precedence, received_at: Time.current,
      read: false, has_attachment: false
    )
  end

  it "allows a human ask in English" do
    email = build_email(subject: "Contract", body: "<p>Please review the attached contract.</p>")

    expect(described_class.email_allows?(email)).to be_truthy
  end

  it "allows a human ask in Portuguese (the old English-only list skipped these)" do
    email = build_email(
      subject: "Documentos contabilidade",
      body: "<p>Bom dia! Podes enviar os documentos da contabilidade até sexta?</p>"
    )

    expect(described_class.email_allows?(email)).to be_truthy
  end

  it "skips FYI mail with no action language" do
    email = build_email(subject: "Extrato Combinado", body: "<p>O seu extrato mensal.</p>")

    expect(described_class.email_allows?(email)).to be_falsey
  end

  it "vetoes machine categories however imperative the wording" do
    email = build_email(
      subject: "Re: [org/repo] PR review", body: "<p>Please fix the failing check.</p>",
      category: "notifications"
    )

    expect(described_class.email_allows?(email)).to be_falsey
    expect(described_class.vetoed?(email)).to be_truthy
  end

  it "vetoes no-reply senders even in a human-looking category" do
    email = build_email(
      subject: "Leave feedback for the seller", body: "<p>Please leave feedback!</p>",
      from: "Team Vinted <no-reply@vinted.example>", category: "personal"
    )

    expect(described_class.email_allows?(email)).to be_falsey
  end

  it "vetoes outbound mail even when From carries a display name" do
    email = build_email(
      subject: "Please review", body: "<p>Please review the draft.</p>",
      from: "Mailbox Owner <mailbox@example.com>"
    )

    expect(email.outbound?).to be_truthy
    expect(described_class.email_allows?(email)).to be_falsey
  end

  it "vetoes Precedence: junk" do
    email = build_email(body: "<p>Please confirm your win!</p>", precedence: "junk")

    expect(described_class.email_allows?(email)).to be_falsey
  end

  it "screens replies on what the sender just wrote, not the quoted history" do
    email = build_email(
      subject: "Re: contract",
      body: "<div>Just bumping this.</div><blockquote>Please review the contract.</blockquote>"
    )

    expect(described_class.email_allows?(email)).to be_falsey
  end
end
