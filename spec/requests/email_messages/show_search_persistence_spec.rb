require "rails_helper"

# Opening an email is a full navigation to the show page; it used to rebuild the
# list pane from the folder scope, silently dropping whatever search/filter was
# active. These specs lock in that the show page now honors search params carried
# on the open link, so the filtered list (and the search box) survive the click.
RSpec.describe "Opening an email keeps the active filter", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    # Keep the show render offline — no live mail-client folder listing.
    allow_any_instance_of(EmailMessagesController)
      .to receive(:folder_mappings)
      .and_return({ name_to_ids: {}, id_to_name: {}, id_to_account: {} })
    sign_in(user)
  end

  it "renders the filtered results in the list pane when the open link carries a query" do
    create(:email_message, email_account: account, subject: "Invoice March")
    create(:email_message, email_account: account, subject: "Weekly Newsletter")
    opened = create(:email_message, email_account: account, subject: "Opened Thread Subject")

    get email_message_path(opened, folder_id: "all", q: "invoice")

    expect(response).to have_http_status(:ok)
    # The list pane is the flat *search* results, not the folder thread list.
    expect(response.body).to include("email_search_list")
    expect(response.body).not_to include('id="email_threads"')
    # Only the matching message shows in the list; the non-match is filtered out.
    expect(response.body).to include("Invoice March")
    expect(response.body).not_to include("Weekly Newsletter")
  end

  it "repopulates the search box from the carried query" do
    opened = create(:email_message, email_account: account, subject: "Opened Thread Subject")

    get email_message_path(opened, folder_id: "all", q: "invoice")

    expect(response.body).to include('value="invoice"')
  end

  it "renders the normal folder list when no filter is carried" do
    create(:email_message, email_account: account, subject: "Plain Inbox Item")
    opened = create(:email_message, email_account: account, subject: "Opened Thread Subject")

    get email_message_path(opened, folder_id: "all")

    expect(response).to have_http_status(:ok)
    expect(response.body).to include('id="email_threads"')
    expect(response.body).not_to include("email_search_list")
  end

  it "carries the active filter onto each result link so the next click keeps it" do
    create(:email_message, email_account: account, subject: "Invoice March")

    get search_email_messages_path(q: "invoice")

    # The result anchor points back at the message *with* the query preserved.
    expect(response.body).to match(%r{/email_messages/\d+\?[^"]*q=invoice})
  end
end
