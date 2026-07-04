class AddRecurrenceToTasks < ActiveRecord::Migration[8.1]
  def change
    # The RFC 5545 RRULE for a recurring task (nil = one-off). Same curated preset
    # strings the calendar events use — see Recurrence / Campbooks::RecurrencePicker.
    add_column :tasks, :rrule, :string

    # Occurrences of a recurring task link (flat) to the series root so completing
    # one can spawn the next and future "edit the whole series" can find its
    # siblings. Nullified if the root is deleted (the orphan becomes its own root).
    add_reference :tasks, :recurrence_parent, type: :uuid, null: true, index: true,
                  foreign_key: { to_table: :tasks, on_delete: :nullify }
  end
end
