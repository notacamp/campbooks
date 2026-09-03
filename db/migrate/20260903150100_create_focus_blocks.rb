# frozen_string_literal: true

# A block of focus time Scout proposes (or the user keeps) for a deadline the AI
# found in mail. Time::FocusProposer places a `proposed` block in open calendar
# space; the user Keeps it (→ a real CalendarEvent on a writable calendar, id in
# calendar_event_id), Moves it to another free slot (`moved`), or dismisses it
# (`dismissed`). Part of the bold Time agenda (Features.bold_layout?).
class CreateFocusBlocks < ActiveRecord::Migration[8.1]
  def change
    create_table :focus_blocks, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.references :user, null: false, foreign_key: true, type: :uuid
      # The deadline the block was held for (proposer source); nullable so a block
      # can outlive a dismissed reminder and to leave room for task-based blocks.
      # index: false — the unique partial index below is the only one we need.
      t.references :reminder, null: true, foreign_key: true, type: :uuid, index: false
      t.references :task, null: true, foreign_key: true, type: :uuid
      # Set once a proposed block is Kept and a writable calendar exists.
      t.references :calendar_event, null: true, foreign_key: true, type: :uuid
      t.string :title, null: false
      t.datetime :start_at, null: false
      t.datetime :end_at, null: false
      t.integer :status, null: false, default: 0
      t.string :reason

      t.timestamps
    end

    add_index :focus_blocks, %i[workspace_id status start_at]
    add_index :focus_blocks, %i[user_id start_at]
    # One block per reminder ever — the proposer's idempotency guard. A dismissed
    # block therefore stays dismissed (never re-proposed), respecting the user.
    add_index :focus_blocks, :reminder_id, unique: true,
      where: "reminder_id IS NOT NULL", name: "index_focus_blocks_on_reminder"
  end
end
