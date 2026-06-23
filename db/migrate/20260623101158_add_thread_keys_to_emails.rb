class AddThreadKeysToEmails < ActiveRecord::Migration[8.1]
  # Frozen copy of Emails::SubjectNormalizer as of this migration. A migration
  # must not couple to app code that can change underneath it.
  PREFIX_RUN = %r{
    \A\s*
    (?:
      \[[^\]]*\]\s*
      |
      (?:re|r[ée]p|fwd?|aw|antw(?:ort)?|sv|vs|wg|tr|enc|rv|odp)
      \s*(?:\[\d+\])?\s*[:：]\s*
    )+
  }ix

  # Lightweight AR shim so the backfill is independent of the EmailThread model.
  class Thread < ActiveRecord::Base
    self.table_name = "email_threads"
  end

  def subject_key(subject)
    subject.to_s.sub(PREFIX_RUN, "").strip.downcase.gsub(/\s+/, " ").strip
  end

  def up
    # Provider-native conversation id (Gmail threadId / Graph conversationId),
    # captured at ingest; NULL for providers that don't expose one (Zoho).
    add_column :email_messages, :provider_thread_id, :string, if_not_exists: true
    add_index  :email_messages, [ :email_account_id, :provider_thread_id ],
               name: "index_email_messages_on_account_and_provider_thread", if_not_exists: true

    add_column :email_threads, :provider_thread_id, :string, if_not_exists: true
    add_column :email_threads, :subject_key, :string, if_not_exists: true
    # NON-unique for now: existing data still has split duplicates. The unique
    # partial indexes are added by a later migration, AFTER `rake emails:merge_split_threads`.
    add_index  :email_threads, [ :email_account_id, :subject_key ],
               name: "index_email_threads_on_account_and_subject_key", if_not_exists: true

    say_with_time "Backfilling email_threads.subject_key" do
      count = 0
      Thread.reset_column_information
      Thread.in_batches(of: 1000) do |batch|
        batch.each do |t|
          t.update_columns(subject_key: subject_key(t.subject))
          count += 1
        end
      end
      count
    end
  end

  def down
    remove_index  :email_threads, name: "index_email_threads_on_account_and_subject_key", if_exists: true
    remove_column :email_threads, :subject_key, if_exists: true
    remove_column :email_threads, :provider_thread_id, if_exists: true
    remove_index  :email_messages, name: "index_email_messages_on_account_and_provider_thread", if_exists: true
    remove_column :email_messages, :provider_thread_id, if_exists: true
  end
end
