# frozen_string_literal: true

# An unsent email being written (or parked) by one user. Both compose surfaces —
# the Dock (bottom sheet) and the Desk (full page) — autosave into the same
# record, which is what lets a draft minimize to a pill, survive navigation, and
# move losslessly between surfaces. Drafts are private to their author (even on
# shared inboxes) and are destroyed on successful send or explicit discard.
class DraftEmail < ApplicationRecord
  # Hygiene cap: autosave prunes the oldest drafts beyond this per user.
  MAX_PER_USER = 20

  belongs_to :workspace
  belongs_to :user
  belongs_to :email_account, optional: true
  belongs_to :in_reply_to, class_name: "EmailMessage", optional: true
  belongs_to :signature, optional: true

  enum :mode, { new_message: 0, reply: 1, reply_all: 2, forward: 3 }, default: :new_message

  scope :latest_first, -> { order(updated_at: :desc) }

  # The pill resumes the most recently edited draft. A row only exists once the
  # user actually typed — the autosave controller creates it on first input, so
  # an opened-and-abandoned reply never becomes a draft. A dismissed draft keeps
  # its content but grows no pill until it's edited again (autosave clears the
  # dismissal on update).
  def self.resumable_for(user)
    where(user: user, dismissed_at: nil).latest_first.first
  end

  def self.prune_for(user)
    ids = where(user: user).latest_first.offset(MAX_PER_USER).pluck(:id)
    where(id: ids).destroy_all if ids.any?
  end

  # What the minimized pill shows: the subject, else the first recipient, in
  # that order — mirroring how a human names an unfinished email.
  def display_title
    return subject if subject.present?

    to_address.to_s.split(",").first&.strip.presence
  end

  # Attachment chips restored on resume: [{ "signed_id", "filename", "byte_size" }].
  def attachment_entries
    Array(attachments_json).filter_map do |entry|
      next unless entry.is_a?(Hash) && entry["signed_id"].present?

      entry.slice("signed_id", "filename", "byte_size")
    end
  end
end
