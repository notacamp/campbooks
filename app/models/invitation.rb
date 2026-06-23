class Invitation < ApplicationRecord
  belongs_to :workspace
  belongs_to :invited_by, class_name: "User"
  belongs_to :accepted_by, class_name: "User", optional: true

  enum :status, { pending: 0, accepted: 1, expired: 2, cancelled: 3 }

  before_create :generate_token
  before_create :set_expires_at

  validates :email, presence: true, format: { with: URI::MailTo::EMAIL_REGEXP }
  validate :email_not_already_member, on: :create
  validate :no_duplicate_pending_invitation, on: :create

  scope :active, -> { pending.where(admin_approved: true).where("expires_at > ?", Time.current) }
  scope :chronological, -> { order(created_at: :desc) }
  scope :pending_admin_approval, -> { pending.where(admin_approved: false) }

  def expired?
    pending? && expires_at < Time.current
  end

  def accept!(user)
    transaction do
      user.update!(workspace: workspace)
      update!(status: :accepted, accepted_by: user, accepted_at: Time.current)
    end
  end

  def cancel!
    update!(status: :cancelled)
  end

  def resend!
    update!(
      token: generate_token,
      expires_at: 7.days.from_now,
      status: :pending
    )
  end

  def approve_by_admin!
    update!(admin_approved: true)
    InvitationMailer.invitation(self).deliver_later
  end

  private

  def generate_token
    self.token = SecureRandom.urlsafe_base64(32)
  end

  def set_expires_at
    self.expires_at ||= 7.days.from_now
  end

  def email_not_already_member
    return unless workspace
    if workspace.users.exists?(email_address: email)
      errors.add(:email, :already_member)
    end
  end

  def no_duplicate_pending_invitation
    return unless workspace && email.present?
    if workspace.invitations.pending.where(email: email).exists?
      errors.add(:email, :already_invited)
    end
  end
end
