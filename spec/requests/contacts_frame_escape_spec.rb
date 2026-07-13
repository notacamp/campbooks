require "rails_helper"

# Regression (#288): the contacts browse list renders inside the `isc_list`
# Turbo Frame (app/views/contacts/index.html.erb). The row link that opens a
# contact must carry data-turbo-frame="_top", or Turbo navigates the frame to a
# page without it and shows "Content missing".
RSpec.describe "Contacts list frame escape", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { sign_in(user) }

  it "opens a contact outside the isc_list frame" do
    person = create(:person, workspace: workspace, name: "Maple Lodge")
    contact = create(:contact, workspace: workspace, email: "maple@example.com",
                               person: person, email_count: 2, last_email_at: Time.current)

    get contacts_path

    expect(response).to have_http_status(:ok)
    anchors = response.body.scan(/<a\s[^>]*>/)
    row_links = anchors.select { |a| a.include?(%(href="/contacts/#{contact.id}")) }
    expect(row_links).not_to be_empty, "no row link to /contacts/#{contact.id} found"
    row_links.each do |anchor|
      expect(anchor).to include('data-turbo-frame="_top"'),
        "expected #{anchor} to escape the isc_list frame with data-turbo-frame=\"_top\""
    end
  end
end
