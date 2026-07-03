require "test_helper"

class WaitingOnRepliesDigestJobTest < ActiveSupport::TestCase
  include ActiveJob::TestHelper

  test "fans out a per-user job only for opted-in users with a workspace" do
    ws = Workspace.create!(name: "Sweep Digest WS")
    opted_in  = ws.users.create!(name: "In", email_address: "sweep-in@example.com", password: "changeme123")
    opted_out = ws.users.create!(name: "Out", email_address: "sweep-out@example.com", password: "changeme123",
                                 email_on_waiting_on_replies_digest: false)

    WaitingOnRepliesDigestJob.perform_now

    fanned_out = enqueued_jobs.select { |job| job[:job] == WaitingOnRepliesDigestMailJob }
                              .map { |job| job[:args].first }
    assert_includes fanned_out, opted_in.id
    refute_includes fanned_out, opted_out.id
  end
end
