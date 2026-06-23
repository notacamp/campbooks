module Emails
  # Single source of truth for grouping an EmailMessage into its EmailThread.
  # Every site that needs a thread for a message goes through here so they can
  # never drift (the bug this replaces: three call sites each did
  # `find_or_create_by(subject:)` with a weak, case-sensitive, single-prefix
  # normalizer, so one conversation split into many threads).
  #
  # Match precedence:
  #   1. provider_thread_id — the provider's own conversation id (Gmail threadId,
  #      Graph conversationId). RFC-correct, sender-agnostic. Best when present.
  #   2. subject_key — normalized subject (Emails::SubjectNormalizer). The only
  #      signal for providers without a thread id (Zoho) and for legacy mail.
  #
  # Race-safe: a unique index backs each branch, so a concurrent insert raises
  # RecordNotUnique and we retry into the now-existing row.
  module Threading
    module_function

    def find_or_create(email)
      account_id = email.email_account_id
      ptid       = email.provider_thread_id.presence
      skey       = email.thread_subject_key

      if ptid
        EmailThread.find_by(email_account_id: account_id, provider_thread_id: ptid) ||
          adopt_legacy_thread(account_id, skey, ptid) ||
          create_thread(account_id, skey, email.thread_subject, ptid)
      else
        # subject_key is a heuristic, NOT unique (two distinct conversations can
        # legitimately share a generic subject) — so match the oldest existing
        # thread deterministically rather than relying on a unique constraint.
        EmailThread.where(email_account_id: account_id, subject_key: skey, provider_thread_id: nil)
                   .order(:id).first ||
          create_thread(account_id, skey, email.thread_subject, nil)
      end
    rescue ActiveRecord::RecordNotUnique
      retry
    end

    # Cutover bridge: a reply to a conversation that existed before provider-id
    # threading shipped has a provider_thread_id, but its thread row predates it.
    # Adopt that subject-keyed thread (claiming it for this provider id) instead of
    # spawning a parallel one. Only threads no provider id has claimed are eligible.
    def adopt_legacy_thread(account_id, skey, ptid)
      return nil if skey.blank?

      thread = EmailThread.where(email_account_id: account_id, subject_key: skey, provider_thread_id: nil)
                          .order(:created_at).first
      thread&.update_columns(provider_thread_id: ptid)
      thread
    end

    def create_thread(account_id, skey, subject, ptid)
      EmailThread.create!(
        email_account_id: account_id,
        subject: subject,
        subject_key: skey,
        provider_thread_id: ptid
      )
    end

    # For outbound sends (workflow action, compose) that start from a raw subject
    # with no incoming message — so a reply we send lands on the same thread the
    # provider/inbound mail will match. Subject-key only (no provider id on send).
    def find_or_create_outbound(account, subject)
      skey    = Emails::SubjectNormalizer.key(subject.to_s)
      display = Emails::SubjectNormalizer.display(subject.to_s).presence || subject.to_s
      EmailThread.where(email_account_id: account.id, subject_key: skey, provider_thread_id: nil)
                 .order(:id).first ||
        create_thread(account.id, skey, display, nil)
    rescue ActiveRecord::RecordNotUnique
      retry
    end
  end
end
