require "rails_helper"

# Covers the close/cancel controls on the event edit view. The view is normally
# loaded as a Turbo Frame inside the calendar's modal, where the
# `calendar-event-modal` Stimulus controller intercepts the click and closes the
# <dialog>. But the same view also renders as a standalone full page (a direct URL
# visit, a Cmd+click on an event chip, or the show -> edit redirect) where that
# controller isn't mounted — so the controls must be real links back to the
# calendar, not JS-only buttons that would otherwise do nothing. This guards that
# server-rendered contract; the in-modal close behaviour lives in the JS controller.
RSpec.describe "Calendar event close/cancel controls", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:calendar_account, workspace: workspace) }
  let(:calendar) { create(:calendar, calendar_account: account) }
  let(:event) { create(:calendar_event, calendar: calendar) }

  before do
    create(:calendar_account_user, :editor, user: user, calendar_account: account)
    sign_in(user)
  end

  # Both the modal-header "X" and the form "Cancel" carry this Stimulus action.
  def close_controls(selector)
    Nokogiri::HTML(response.body).css("#{selector}[data-action~='click->calendar-event-modal#close']")
  end

  it "renders the close/cancel controls as links to the calendar, not dead buttons" do
    get edit_calendar_event_path(event)

    expect(response).to have_http_status(:ok)
    # The bug: these were <button>s, inert without the modal controller. They must
    # now be <a>s so a full-page visit can still get back to the calendar.
    expect(close_controls("button")).to be_empty
    links = close_controls("a")
    expect(links.size).to be >= 2 # the header X and the form Cancel

    links.each do |link|
      expect(link["href"]).to eq(calendar_path)
      # Break out of the calendar_event_modal frame on a full-page render.
      expect(link["data-turbo-frame"]).to eq("_top")
    end
  end

  it "preserves the active view in the close link so you land back on the same view" do
    get edit_calendar_event_path(event, view: "week")

    expect(response).to have_http_status(:ok)
    close_controls("a").each do |link|
      expect(link["href"]).to eq(calendar_path(view: "week"))
    end
  end
end
