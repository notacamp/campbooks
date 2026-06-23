require "rails_helper"

RSpec.describe BugReportGithubSyncJob do
  let(:bug_report) { create(:bug_report) }

  describe ".configured?" do
    before { allow(ENV).to receive(:[]).and_call_original }

    it "is false without a token" do
      allow(ENV).to receive(:[]).with("GITHUB_TOKEN").and_return(nil)
      expect(described_class).not_to be_configured
    end

    it "is true with a token and a repo" do
      allow(ENV).to receive(:[]).with("GITHUB_TOKEN").and_return("tok")
      allow(ENV).to receive(:[]).with("GITHUB_BUG_REPORT_REPO").and_return("acme/app")
      expect(described_class).to be_configured
    end
  end

  describe "#perform" do
    context "when GitHub is not configured" do
      it "is a no-op" do
        allow(described_class).to receive(:configured?).and_return(false)
        expect(Workflows::HttpClient).not_to receive(:call)

        described_class.new.perform(bug_report.id)

        expect(bug_report.reload.github_issue_number).to be_nil
      end
    end

    context "when GitHub is configured" do
      before do
        allow(described_class).to receive(:configured?).and_return(true)
        allow(described_class).to receive(:repo).and_return("acme/app")
        allow(ENV).to receive(:fetch).and_call_original
        allow(ENV).to receive(:fetch).with("GITHUB_TOKEN").and_return("tok")
      end

      it "opens an issue and stores the number + url" do
        allow(Workflows::HttpClient).to receive(:call).and_return(
          ok: true, status: 201, headers: {}, error: nil,
          body: { number: 99, html_url: "https://github.com/acme/app/issues/99" }.to_json
        )

        described_class.new.perform(bug_report.id)

        expect(bug_report.reload.github_issue_number).to eq(99)
        expect(bug_report.github_issue_url).to eq("https://github.com/acme/app/issues/99")
      end

      it "posts to the configured repo's issues endpoint with auth" do
        allow(Workflows::HttpClient).to receive(:call).and_return(
          ok: true, status: 201, headers: {}, error: nil, body: { number: 1, html_url: "x" }.to_json
        )

        described_class.new.perform(bug_report.id)

        expect(Workflows::HttpClient).to have_received(:call).with(
          hash_including(
            method: :post,
            url: "https://api.github.com/repos/acme/app/issues",
            headers: hash_including("Authorization" => "Bearer tok")
          )
        )
      end

      it "raises on API failure so the job retries" do
        allow(Workflows::HttpClient).to receive(:call).and_return(
          ok: false, status: 403, headers: {}, error: nil, body: "Forbidden"
        )

        expect { described_class.new.perform(bug_report.id) }
          .to raise_error(/GitHub issue creation failed/)
      end

      it "skips a report that is already synced" do
        synced = create(:bug_report, :synced)
        expect(Workflows::HttpClient).not_to receive(:call)

        described_class.new.perform(synced.id)
      end
    end
  end
end
