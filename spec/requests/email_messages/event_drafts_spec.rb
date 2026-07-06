require "rails_helper"

# EmailMessages::EventDraftsController — lazy turbo-frame + "Add to calendar"
# Contract:
#  GET  show — returns an empty frame when preconditions are not met;
#              returns EventDraftBlock in :draft state when a time is found.
#  POST create — calls Tools::CreateCalendarEvent, returns turbo_stream with
#               :confirmed or :error block.
RSpec.describe "Email message event drafts", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:message)   { create(:email_message, email_account: account, subject: "Call at 3pm tomorrow", body: "Does 3pm work for a call?") }

  let(:cal_account) { create(:calendar_account, workspace: workspace) }
  let!(:calendar) do
    create(:calendar, :primary, calendar_account: cal_account,
           is_writable: true, syncing: true)
  end

  before do
    create(:email_account_user, :collaborator, user: user, email_account: account)
    create(:calendar_account_user, :editor, user: user, calendar_account: cal_account)
    sign_in(user)
  end

  # ── GET show ────────────────────────────────────────────────────────────────

  describe "GET event_draft" do
    it "returns a matching turbo-frame with the draft block when time is detected" do
      get event_draft_email_message_path(message)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("event_draft_#{message.id}")
      expect(response.body).to include("drafted from this email")
      # Edit and Add buttons are present
      expect(response.body).to include("Add to calendar")
      expect(response.body).to include("Edit")
    end

    it "returns an empty turbo-frame when the email has no time mention" do
      no_time_msg = create(:email_message,
                           email_account: account,
                           subject: "Let us meet",
                           body: "Hello, we should get together sometime next week.")
      get event_draft_email_message_path(no_time_msg)

      expect(response).to have_http_status(:ok)
      # Frame tag is present but content is empty
      expect(response.body).to include("event_draft_#{no_time_msg.id}")
      expect(response.body).not_to include("drafted from this email")
    end

    it "returns an empty turbo-frame when the user has no writable calendar" do
      # Remove the writable calendar
      calendar.update!(is_writable: false)

      get event_draft_email_message_path(message)

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include("drafted from this email")
    end

    it "returns an empty turbo-frame for an email sent by the user (outbound)" do
      sent_msg = create(:email_message,
                        email_account: account,
                        from_address: account.email_address,
                        subject: "My proposal at 2pm",
                        body: "I suggested 2pm.")
      get event_draft_email_message_path(sent_msg)

      expect(response).to have_http_status(:ok)
      expect(response.body).not_to include("drafted from this email")
    end

    it "returns 404 for a message the user cannot access" do
      other_account  = create(:email_account, workspace: workspace)
      other_message  = create(:email_message, email_account: other_account, subject: "Meeting at 2pm", body: "Let us meet at 2pm")

      get event_draft_email_message_path(other_message)
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── POST create ─────────────────────────────────────────────────────────────

  describe "POST event_draft" do
    before do
      # Stub EventWriteJob so the spec does not need live provider creds
      allow(Calendars::EventWriteJob).to receive(:perform_later)
      allow(EventClassificationJob).to receive(:set).and_return(
        double("job", perform_later: true)
      )
    end

    it "creates a CalendarEvent and returns a turbo_stream with the confirmed block" do
      expect {
        post event_draft_email_message_path(message), as: :turbo_stream
      }.to change(CalendarEvent, :count).by(1)

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("turbo-stream")
      expect(response.body).to include("On your calendar")
    end

    it "links source_email_message on the created event" do
      post event_draft_email_message_path(message), as: :turbo_stream

      event = CalendarEvent.order(:created_at).last
      expect(event.source_email_message).to eq(message)
    end

    it "returns turbo_stream with the error block when no calendar is available" do
      calendar.update!(is_writable: false)

      post event_draft_email_message_path(message), as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("turbo-stream")
      # Error block or empty (no calendar returns the EmailActions failure message)
      expect(response.body).not_to include("On your calendar")
    end

    it "returns 404 for a message the user cannot access" do
      other_account  = create(:email_account, workspace: workspace)
      other_message  = create(:email_message, email_account: other_account, subject: "Meeting at 2pm", body: "2pm works?")

      post event_draft_email_message_path(other_message), as: :turbo_stream
      expect(response).to have_http_status(:not_found)
    end
  end
end
