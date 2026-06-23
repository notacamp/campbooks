require "rails_helper"

# The compose action pre-fills the To field. Reply uses the original sender;
# reply-all adds the other recipients but must drop the sending account's own
# address — compared by its bare email so a "Display Name <addr>" form is still
# recognized as self (otherwise the user ends up emailing themselves).
RSpec.describe "EmailCompose recipient pre-fill", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace, email_address: "me@example.com") }

  before do
    create(:email_account_user, user: user, email_account: account) # can_read by default
    sign_in(user)
  end

  # The To field is a hidden input seeded server-side; pull its value back out.
  def to_field_value(body)
    body[/name="to_address"\s+value="([^"]*)"/, 1]
  end

  it "pre-fills reply with the original sender" do
    message = create(:email_message,
      email_account: account,
      from_address: "Sender <sender@example.com>",
      to_address: "me@example.com")

    post compose_email_message_path(message), params: { mode: "reply" }, as: :turbo_stream

    expect(response).to have_http_status(:ok)
    expect(to_field_value(response.body)).to include("sender@example.com")
  end

  it "keeps every other party but drops the user's own address from reply-all" do
    message = create(:email_message,
      email_account: account,
      from_address: "Sender <sender@example.com>",
      to_address: "Me <me@example.com>, Other <other@example.com>")

    post compose_email_message_path(message), params: { mode: "reply_all" }, as: :turbo_stream

    expect(response).to have_http_status(:ok)
    to = to_field_value(response.body)
    expect(to).to include("sender@example.com")
    expect(to).to include("other@example.com")
    expect(to).not_to include("me@example.com")
  end

  # "Edit in composer" from a Scout draft opens the composer pre-filled and
  # removes the preview card.
  it "pre-fills the composer with the Scout draft body and removes the preview card" do
    message = create(:email_message, email_account: account, from_address: "sender@example.com", subject: "Q")

    post compose_email_message_path(message),
      params: { mode: "reply", body: "Scout drafted this reply.", remove_draft: "draft_email_message_#{message.id}", compose_target: "thread_compose_target" },
      as: :turbo_stream

    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Scout drafted this reply.")
    expect(response.body).to include("draft_email_message_#{message.id}") # the remove target
  end
end

# Sending is provider-neutral: Gmail/Zoho expose a one-shot send_message, but
# Microsoft Graph does not — there the controller must save a draft then send it.
RSpec.describe "EmailCompose provider-safe send", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace, email_address: "me@example.com") }
  let(:message) { create(:email_message, email_account: account, from_address: "sender@example.com", subject: "Q") }

  before do
    create(:email_account_user, :collaborator, user: user, email_account: account) # can_send
    sign_in(user)
  end

  it "falls back to save_draft + send_draft when the client has no send_message" do
    client = instance_double("Microsoft::MailClient")
    allow(client).to receive(:save_draft).and_return({ "id" => "draft-1" })
    allow(client).to receive(:send_draft).and_return(true)
    allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(client)

    post send_message_email_message_path(message),
      params: { to_address: "sender@example.com", subject: "Re: Q", body: "Hello" }, as: :turbo_stream

    expect(response).to have_http_status(:ok)
    expect(client).to have_received(:save_draft)
    expect(client).to have_received(:send_draft).with("draft-1")
    expect(account.email_messages.where(provider_folder_id: "sent")).to exist
  end
end
