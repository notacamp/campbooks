# Preview for Campbooks::FollowButton — the Follow / Following toggle in the
# email discussion panel header.
class FollowButtonComponentPreview < ViewComponent::Preview
  def not_following
    render Campbooks::FollowButton.new(email_message: EmailMessage.new(id: 123), following: false)
  end

  def following
    render Campbooks::FollowButton.new(email_message: EmailMessage.new(id: 123), following: true)
  end
end
