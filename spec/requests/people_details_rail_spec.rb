# frozen_string_literal: true

require "rails_helper"

# The details rail beside a People conversation scrolls on its own and can be
# collapsed at desktop width: the aside is a scroll container, the ⓘ button in the
# conversation header is present at every width (it collapses/expands the rail at
# xl and toggles the sheet below), and the rail's own header carries a close control.
RSpec.describe "People details rail", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    sign_in(user)
  end

  let(:person) { create(:person, workspace: workspace, name: "Sofia Martins") }
  let!(:contact) do
    create(:contact, workspace: workspace, email_account: account, person: person,
           email: "sofia@brightloop.example", sender_kind: :person, email_count: 2)
  end

  it "renders the rail as its own scroll container with a details button at every width" do
    thread = create(:email_thread, email_account: account, subject: "Q3 deck")
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
           from_address: "sofia@brightloop.example", body: "Hi", received_at: 1.day.ago)

    get person_page_path(person)

    expect(response).to have_http_status(:ok)
    aside = response.body[/<aside id="people_details_pane"[^>]*>/]
    expect(aside).to include("overflow-y-auto")
    # `[^<]` rather than `[^>]`: the button's data-action contains "->".
    button = response.body[/<button[^<]*?data-people-details-target="detailsBtn"[^<]*?>/]
    expect(button).to be_present
    expect(button).not_to include("xl:hidden")
  end

  it "gives the rail header a close control that works at desktop width too" do
    get people_details_path(person)

    expect(response).to have_http_status(:ok)
    header = response.body[/<div class="flex items-center justify-between border-b[^>]*>/]
    expect(header).not_to include("xl:hidden")
    expect(response.body).to include(%(aria-label="#{I18n.t("components.people.details.close")}"))
    expect(response.body).to include("people-details#close")
  end
end
