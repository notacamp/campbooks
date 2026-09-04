require "rails_helper"

RSpec.describe "Time (bold layout)", type: :request do
  let(:user) { create(:user) }
  let(:workspace) { user.workspace }

  before { sign_in(user) }

  it "renders the agenda with a document-sourced deadline (the v0.32.0 /time 500)" do
    document = create(:document, workspace: workspace).tap { |d| d.assign_title("Seguro Renovação 2026"); d.save! }
    create(:reminder, workspace: workspace, source: document, reminder_type: :renewal,
                      title: "Policy renewal", due_at: 2.days.from_now, all_day: true)

    get time_path
    expect(response).to have_http_status(:ok)
    expect(response.body).to include("Policy renewal")
    expect(response.body).to include("Seguro Renovação 2026")
  end

  it "renders the week and month views" do
    get time_path(view: "week")
    expect(response).to have_http_status(:ok)
    get time_path(view: "month")
    expect(response).to have_http_status(:ok)
  end
end
