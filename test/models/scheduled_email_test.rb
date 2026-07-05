require "test_helper"

# Queued sends are outbound mail on a specific mailbox — they must follow the
# mailbox's sharing, not be workspace-public: readable by the creator and by
# mailbox readers, mutable only by the creator or someone with send access.
class ScheduledEmailTest < ActiveSupport::TestCase
  setup do
    @workspace = create(:workspace)
    @creator = create(:user, workspace: @workspace)
    @account = create(:email_account, workspace: @workspace)
    create(:email_account_user, :owner, user: @creator, email_account: @account)
    @scheduled = create(:scheduled_email, workspace: @workspace, email_account: @account, created_by: @creator)
  end

  test "accessible_to includes the creator even without a mailbox share" do
    other_account = create(:email_account, workspace: @workspace)
    orphaned = create(:scheduled_email, workspace: @workspace, email_account: other_account, created_by: @creator)

    assert_includes ScheduledEmail.accessible_to(@creator), orphaned
  end

  test "accessible_to includes mailbox readers" do
    reader = create(:user, workspace: @workspace)
    create(:email_account_user, :viewer, user: reader, email_account: @account)

    assert_includes ScheduledEmail.accessible_to(reader), @scheduled
  end

  test "accessible_to excludes workspace members with no share on the mailbox" do
    bystander = create(:user, workspace: @workspace)

    assert_empty ScheduledEmail.accessible_to(bystander)
  end

  test "accessible_to excludes users from another workspace and nil users" do
    outsider = create(:user)

    assert_empty ScheduledEmail.accessible_to(outsider)
    assert_empty ScheduledEmail.accessible_to(nil)
  end

  test "editable_by the creator and by senders, not by read-only sharees" do
    reader = create(:user, workspace: @workspace)
    create(:email_account_user, :viewer, user: reader, email_account: @account)
    sender = create(:user, workspace: @workspace)
    create(:email_account_user, :collaborator, user: sender, email_account: @account)

    assert @scheduled.editable_by?(@creator)
    assert @scheduled.editable_by?(sender)
    assert_not @scheduled.editable_by?(reader)
    assert_not @scheduled.editable_by?(nil)
  end
end
