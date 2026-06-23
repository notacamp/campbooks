class SignupRequest < ApplicationRecord
  enum :status, { pending: 0, approved: 1, rejected: 2, accepted: 3 }

  belongs_to :reviewed_by, class_name: "User", optional: true
  belongs_to :accepted_by, class_name: "User", optional: true

  before_create :generate_token

  validates :email, presence: true, format: { with: URI::MailTo::EMAIL_REGEXP }
  validates :name, presence: true

  scope :pending_review, -> { pending.order(created_at: :asc) }
  scope :by_status, ->(status) { where(status: status) if status.present? }
  scope :chronological, -> { order(created_at: :desc) }

  def approve!(admin)
    update!(status: :approved, reviewed_by: admin, reviewed_at: Time.current)
    SignupRequestMailer.approved(self).deliver_later
  end

  def reject!(admin)
    update!(status: :rejected, reviewed_by: admin, reviewed_at: Time.current)
  end

  def accept!(user)
    update!(status: :accepted, accepted_by: user)
  end

  private

  def generate_token
    self.token = SecureRandom.urlsafe_base64(32)
  end
end
