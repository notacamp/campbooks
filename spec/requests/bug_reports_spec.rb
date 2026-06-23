require "rails_helper"

RSpec.describe "BugReports", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "POST /bug_reports" do
    it "does not create a report when unauthenticated" do
      expect {
        post bug_reports_path, params: { description: "anything" }
      }.not_to change(BugReport, :count)
    end

    context "when signed in" do
      before do
        sign_in(user)
        allow(BugReportGithubSyncJob).to receive(:perform_later)
      end

      it "creates a workspace-scoped report for the current user" do
        expect {
          post bug_reports_path,
            params: {
              description: "Skim button does nothing",
              page_url: "https://app.campbooks.not-a-camp.com/feed",
              metadata: { viewport: "375x812", breakpoint: "xs" }.to_json
            },
            headers: { "Accept" => "application/json", "User-Agent" => "Mozilla/5.0 (RSpec)" }
        }.to change(BugReport, :count).by(1)

        expect(response).to have_http_status(:created)
        expect(response.parsed_body["ok"]).to be(true)

        report = BugReport.last
        expect(report.workspace).to eq(workspace)
        expect(report.user).to eq(user)
        expect(report.description).to eq("Skim button does nothing")
        expect(report.page_url).to eq("https://app.campbooks.not-a-camp.com/feed")
        expect(report.context("viewport")).to eq("375x812")
        expect(report.user_agent).to eq("Mozilla/5.0 (RSpec)")
      end

      it "keeps only whitelisted metadata keys" do
        post bug_reports_path,
          params: { description: "hi", metadata: { viewport: "375x812", evil: "drop table" }.to_json },
          headers: { "Accept" => "application/json" }

        expect(BugReport.last.metadata.keys).to contain_exactly("viewport")
      end

      it "returns 422 with errors when the description is blank" do
        post bug_reports_path,
          params: { description: "   " },
          headers: { "Accept" => "application/json" }

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body["ok"]).to be(false)
        expect(response.parsed_body["errors"]).to be_present
      end

      it "enqueues the GitHub sync job when GitHub is configured" do
        allow(BugReportGithubSyncJob).to receive(:configured?).and_return(true)

        post bug_reports_path, params: { description: "hi" }, headers: { "Accept" => "application/json" }

        expect(BugReportGithubSyncJob).to have_received(:perform_later).with(kind_of(Integer))
      end

      it "skips the GitHub sync job when GitHub is not configured" do
        allow(BugReportGithubSyncJob).to receive(:configured?).and_return(false)

        post bug_reports_path, params: { description: "hi" }, headers: { "Accept" => "application/json" }

        expect(BugReportGithubSyncJob).not_to have_received(:perform_later)
      end
    end
  end
end
