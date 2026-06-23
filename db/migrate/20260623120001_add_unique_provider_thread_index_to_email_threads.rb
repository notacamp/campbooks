class AddUniqueProviderThreadIndexToEmailThreads < ActiveRecord::Migration[8.1]
  # Run AFTER `rake emails:merge_split_threads` (no provider_thread_id duplicates
  # can exist when this index is created; historical rows are all NULL, so the
  # partial index covers zero rows until new mail is ingested).
  def up
    # provider_thread_id is the provider's true conversation id — enforce one
    # thread per (account, conversation). subject_key is intentionally NOT unique
    # (distinct conversations may share a generic subject), so it gets no unique index.
    add_index :email_threads, [ :email_account_id, :provider_thread_id ],
              unique: true, where: "provider_thread_id IS NOT NULL",
              name: "index_email_threads_on_account_and_provider_thread_uniq",
              if_not_exists: true

    # The old subject-based uniqueness is wrong now: two distinct conversations
    # can share a display subject (matching is via subject_key/provider id instead).
    remove_index :email_threads,
                 name: "index_email_threads_on_subject_and_email_account_id", if_exists: true
  end

  def down
    add_index :email_threads, [ :subject, :email_account_id ],
              unique: true, name: "index_email_threads_on_subject_and_email_account_id",
              if_not_exists: true
    remove_index :email_threads,
                 name: "index_email_threads_on_account_and_provider_thread_uniq", if_exists: true
  end
end
