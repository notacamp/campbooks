# frozen_string_literal: true

require "test_helper"

# Focused test: verifies that Accounts::Deleter purges the deleted workspace's
# ExternalServiceCall rows without touching other workspaces' rows.
class Accounts::DeleterExternalServiceCallsTest < ActiveSupport::TestCase
  setup do
    @ws_a = create(:workspace)
    @ws_b = create(:workspace)

    @user_a = create(:user, workspace: @ws_a)

    # Seed rows for both workspaces and a nil-workspace row.
    @ws_a_call = create(:external_service_call, workspace: @ws_a)
    @ws_b_call = create(:external_service_call, workspace: @ws_b)
    @nil_call  = ExternalServiceCall.create!(service: "smtp", status: :success, workspace_id: nil)
  end

  test "deleting the sole member of a workspace purges that workspace's ExternalServiceCall rows" do
    assert_difference("ExternalServiceCall.count", -1) do
      Accounts::Deleter.new(@user_a).delete!
    end

    # The ws_a call is gone.
    assert_not ExternalServiceCall.exists?(@ws_a_call.id)
  end

  test "deleting ws_a leaves ws_b and nil-workspace rows intact" do
    Accounts::Deleter.new(@user_a).delete!

    assert ExternalServiceCall.exists?(@ws_b_call.id), "ws_b call must survive"
    assert ExternalServiceCall.exists?(@nil_call.id),  "nil-workspace call must survive"
  end
end
