class AddEventTypeToCalendarEvents < ActiveRecord::Migration[8.1]
  def change
    add_reference :calendar_events, :event_type, foreign_key: true, null: true
    # 0 pending (awaiting AI auto-classification), 1 auto (AI assigned), 2 manual
    # (user picked a type or "none" — never auto-touch again).
    add_column :calendar_events, :type_status, :integer, default: 0, null: false
  end
end
