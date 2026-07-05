require "test_helper"

# The role ladder for shared mailboxes: viewer (read), collaborator
# (read + send), manager (read + send + manage), with `owner` as a separate
# creation-time flag. `role=` must never grant or clear flags for a role name
# it doesn't recognise.
class EmailAccountUserTest < ActiveSupport::TestCase
  setup do
    @workspace = create(:workspace)
    @account = create(:email_account, workspace: @workspace)
    @user = create(:user, workspace: @workspace)
  end

  test "each assignable role maps to its exact flag bundle" do
    entry = create(:email_account_user, user: @user, email_account: @account)

    entry.role = "viewer"
    assert entry.can_read?
    assert_not entry.can_send?
    assert_not entry.can_manage?

    entry.role = "collaborator"
    assert entry.can_read?
    assert entry.can_send?
    assert_not entry.can_manage?

    entry.role = "manager"
    assert entry.can_read?
    assert entry.can_send?
    assert entry.can_manage?
  end

  test "role is derived from flags, with owner taking precedence" do
    assert_equal "viewer", create(:email_account_user, :viewer, user: @user, email_account: @account).role
    assert_equal "collaborator", build(:email_account_user, :collaborator).role
    assert_equal "manager", build(:email_account_user, :manager).role
    assert_equal "owner", build(:email_account_user, :owner).role
  end

  test "an unknown role name changes nothing" do
    entry = create(:email_account_user, :viewer, user: @user, email_account: @account)

    entry.role = "superadmin"

    assert entry.can_read?
    assert_not entry.can_send?
    assert_not entry.can_manage?
    assert_not entry.owner?
  end

  test "a user can only be granted access to an account once" do
    create(:email_account_user, :viewer, user: @user, email_account: @account)
    duplicate = build(:email_account_user, :manager, user: @user, email_account: @account)

    assert_not duplicate.valid?
  end
end
