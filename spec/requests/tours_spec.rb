require "rails_helper"

RSpec.describe "Tours", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "POST /tours/:key/dismiss" do
    it "does not record a tour when unauthenticated" do
      expect {
        post dismiss_tour_path("skim_intro")
      }.not_to(change { user.reload.dismissed_tours })
      expect(response).to have_http_status(:redirect)
    end

    context "when signed in" do
      before { sign_in(user) }

      it "records the tour as dismissed for the current user" do
        expect {
          post dismiss_tour_path("skim_intro")
        }.to change { user.reload.tour_dismissed?("skim_intro") }.from(false).to(true)

        expect(response).to have_http_status(:no_content)
      end

      it "is idempotent across repeated dismissals" do
        2.times { post dismiss_tour_path("doc_skim_intro") }

        expect(user.reload.dismissed_tours).to eq([ "doc_skim_intro" ])
      end
    end
  end
end
