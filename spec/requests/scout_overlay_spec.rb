require "rails_helper"

# The Scout overlay body endpoint — loaded lazily into the overlay's turbo-frame.
RSpec.describe "Scout overlay", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  # A global Scout thread that carries a message shows up under Recent.
  def create_thread(for_user: user, title: "Unpaid invoices")
    thread = create(:agent_thread, user: for_user, workspace: for_user.workspace, purpose: :global, title: title)
    create(:agent_message, agent_thread: thread, user: for_user, content: "Which invoices are unpaid?")
    thread
  end

  before { sign_in(user) }

  it "renders the idle body: suggestions, recent threads, and the command list" do
    allow(Ai::ProviderSetup).to receive(:available?).and_call_original
    allow(Ai::ProviderSetup).to receive(:available?).with(anything, :text).and_return(true)
    create_thread(title: "Unpaid invoices this month")

    get scout_overlay_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include('data-scout-overlay-mode="idle"')
    expect(response.body).to include('data-scout-overlay-target="list"')
    # Recent thread (title + a button that opens it)
    expect(response.body).to include("Unpaid invoices this month")
    expect(response.body).to include("scout-overlay#openThread")
    # Scout::Briefing suggestions render as ask-chips
    expect(response.body).to include("data-chat-input-text-param")
  end

  it "shows the no-AI line and no suggestions when text AI is unavailable" do
    allow(Ai::ProviderSetup).to receive(:available?).and_call_original
    allow(Ai::ProviderSetup).to receive(:available?).with(anything, :text).and_return(false)

    get scout_overlay_path
    expect(response).to have_http_status(:ok)
    # The apostrophe in "can't" is HTML-escaped in the raw body, so match around it.
    expect(response.body).to include("answer questions yet")
    expect(response.body).to include("Add an AI provider in Settings")
    expect(response.body).to include(settings_ai_path)
    expect(response.body).not_to include("data-chat-input-text-param")
  end

  it "renders a thread's conversation with ?thread_id" do
    thread = create_thread(title: "Reply to Miguel")
    create(:agent_message, agent_thread: thread, user: user, author_type: :ai, content: "Here is a draft.")

    get scout_overlay_path(thread_id: thread.id)
    expect(response).to have_http_status(:ok)
    expect(response.body).to include('data-scout-overlay-mode="conversation"')
    expect(response.body).to include('id="agent_messages_list"')
    expect(response.body).to include("Here is a draft.")
    # Subscribed to the reply-job stream (the channel name is signed/encoded).
    expect(response.body).to include("turbo-cable-stream-source")
  end

  it "404s for another user's thread (no existence leak)" do
    other = create(:user, workspace: create(:workspace))
    others_thread = create_thread(for_user: other, title: "Private")

    get scout_overlay_path(thread_id: others_thread.id)
    expect(response).to have_http_status(:not_found)
  end
end
