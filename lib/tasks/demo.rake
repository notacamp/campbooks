# frozen_string_literal: true

namespace :demo do
  desc "Reset the Demo Workspace to a clean, freshly-seeded slate (undo archive/snooze/done/paid from a demo run, then re-seed)"
  task reset: :environment do
    org = Workspace.find_by(slug: "demo")
    abort("No Demo Workspace (slug: demo) — run `bin/rails db:seed` first.") unless org

    # Undo the mutable bits a live demo run dirties — all scoped to the demo
    # workspace, so real data elsewhere is never touched.

    # archive / snooze → move the demo mailbox's messages back to the inbox folder
    # (the seed's re-run only backfills a BLANK folder, so archived ones need this).
    # Tear down the rich-demo records that accumulate across re-seeds or hold
    # per-run state — feed cards, reminders, and the demo mailbox's threads +
    # messages — so the re-seed below recreates EXACTLY the canonical set with no
    # duplicates. People, contacts, documents, tasks, events and the focus block
    # stay (idempotent find_or_create on stable keys) and are re-asserted.
    # refresh_token is encrypted (non-deterministic) so the demo account is
    # selected in Ruby via #demo?, not a query.
    account = org.email_accounts.detect(&:demo?)

    # Atomic teardown so a failure can't leave the demo half-destroyed (db:seed
    # stays OUTSIDE this transaction).
    ActiveRecord::Base.transaction do
      # focus_blocks.reminder_id is the ONLY FK into reminders (the Keep /
      # FocusKeeper link, added at runtime) — nullify it first or Reminder.delete
      # violates the FK. The focus block itself is kept (idempotent seed).
      FocusBlock.where(workspace: org).where.not(reminder_id: nil).update_all(reminder_id: nil)

      FeedItem.where(workspace: org).delete_all
      Reminder.where(workspace: org).delete_all
      if account
        EmailMessage.where(email_account: account).destroy_all
        EmailThread.where(email_account: account).destroy_all
      end

      # paid → un-settle manual settlements on the kept documents (a bank match stays).
      org.documents.where(settled_source: %w[manual elsewhere])
         .update_all(settled_at: nil, settled_source: nil)

      # done / cancelled asks → back to actionable (tasks are kept, not torn down).
      org.tasks.where(status: %i[done cancelled])
         .update_all(status: Task.statuses[:todo], snoozed_until: nil, archived_at: nil)
    end

    # Force the rich-demo block to re-run (fills any gaps), then refresh standings
    # so Today / Inbox reflect the clean slate immediately.
    org.settings.delete("rich_demo_seeded_v1")
    org.save!
    Rake::Task["db:seed"].invoke
    User.where(workspace: org).find_each { |user| People::Standings.refresh!(user) }

    puts "✔ Demo Workspace reset to a clean slate."
  end
end
