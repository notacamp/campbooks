# frozen_string_literal: true

class EmailAccountSharingComponentPreview < ViewComponent::Preview
  # @label Default (owner + collaborator + viewer)
  def default
    render Campbooks::EmailAccountSharing.new(
      account: account,
      members: [ owner_member, collaborator_member, viewer_member ],
      addable_users: [ User.new(id: 4, name: "Dana New", email_address: "dana@acme.com") ],
      current_user: owner_user
    )
  end

  # @label Owner only (nobody shared yet)
  def owner_only
    render Campbooks::EmailAccountSharing.new(
      account: account,
      members: [ owner_member ],
      addable_users: [
        User.new(id: 2, name: "Bob Sender", email_address: "bob@acme.com"),
        User.new(id: 3, name: "Carol Viewer", email_address: "carol@acme.com")
      ],
      current_user: owner_user
    )
  end

  # @label Everyone already has access (no one left to add)
  def everyone_added
    render Campbooks::EmailAccountSharing.new(
      account: account,
      members: [ owner_member, collaborator_member ],
      addable_users: [],
      current_user: owner_user
    )
  end

  private

  def account
    EmailAccount.new(id: 1, email_address: "support@acme.com", name: "Support")
  end

  def owner_user
    @owner_user ||= User.new(id: 1, name: "Alice Owner", email_address: "alice@acme.com")
  end

  def owner_member
    EmailAccountUser.new(user: owner_user, user_id: 1, owner: true, can_read: true, can_send: true, can_manage: true)
  end

  def collaborator_member
    EmailAccountUser.new(
      user: User.new(id: 2, name: "Bob Sender", email_address: "bob@acme.com"),
      user_id: 2, can_read: true, can_send: true
    )
  end

  def viewer_member
    EmailAccountUser.new(
      user: User.new(id: 3, name: "Carol Viewer", email_address: "carol@acme.com"),
      user_id: 3, can_read: true
    )
  end
end
