class AuthoredDocument < ApplicationRecord
  belongs_to :workspace
  belongs_to :author, class_name: "User", optional: true

  # Filed into folders (the Files "filesystem" layer) via the same polymorphic
  # join Document uses, so internal documents organize alongside files and emails.
  has_many :folder_memberships, as: :folderable, dependent: :destroy
  has_many :mail_folders, through: :folder_memberships

  validates :title, presence: true, length: { maximum: 255 }
  validates :html_content, length: { maximum: 500_000 }

  scope :recent, -> { order(created_at: :desc) }
  scope :in_folder, ->(folder_id) { joins(:folder_memberships).where(folder_memberships: { mail_folder_id: folder_id }) }
end
