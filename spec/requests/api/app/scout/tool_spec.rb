# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Scout tool", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  describe "POST /api/app/scout/tool" do
    context "with a valid confirm-level tool" do
      before do
        allow(Scout::ToolRegistry).to receive(:run).with("bulk_archive", anything).and_return(
          { archived_count: 3 }
        )
      end

      it "executes the tool and returns a structured result" do
        post "/api/app/scout/tool",
             params: { tool: "bulk_archive", args: { email_ids: [ "abc" ] }.to_json },
             headers: api_app_headers(user)

        expect(response).to have_http_status(:ok)
        data = response.parsed_body["data"]
        expect(data["tool"]).to eq("bulk_archive")
        expect(data["success"]).to be(true)
        expect(data["toast_message"]).to eq("Archived 3 email(s)")
        expect(data["result"]["archived_count"]).to eq(3)
      end
    end

    context "with a read-only tool (not confirm-level)" do
      it "returns 422 tool_not_allowed" do
        post "/api/app/scout/tool",
             params: { tool: "query_emails", args: {} },
             headers: api_app_headers(user)

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("tool_not_allowed")
      end
    end

    context "with an unknown tool" do
      it "returns 422 tool_not_allowed" do
        post "/api/app/scout/tool",
             params: { tool: "does_not_exist" },
             headers: api_app_headers(user)

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("tool_not_allowed")
      end
    end

    context "when the tool execution fails" do
      before do
        allow(Scout::ToolRegistry).to receive(:run).and_return({ error: "Something went wrong" })
      end

      it "returns 422 tool_failed" do
        post "/api/app/scout/tool",
             params: { tool: "bulk_archive", args: {} },
             headers: api_app_headers(user)

        expect(response).to have_http_status(:unprocessable_entity)
        expect(response.parsed_body.dig("error", "code")).to eq("tool_failed")
        expect(response.parsed_body.dig("error", "message")).to include("Something went wrong")
      end
    end

    it "401s without a token" do
      post "/api/app/scout/tool", params: { tool: "bulk_archive" }
      expect(response).to have_http_status(:unauthorized)
    end
  end
end
