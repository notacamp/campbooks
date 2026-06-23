class CreateBugReports < ActiveRecord::Migration[8.1]
  def change
    create_table :bug_reports do |t|
      t.references :workspace, null: false, foreign_key: true, index: false
      t.references :user, null: false, foreign_key: true
      t.text :description, null: false
      t.integer :status, null: false, default: 0
      t.string :page_url
      t.string :user_agent
      # Captured browser context: viewport, screen, device_pixel_ratio,
      # breakpoint, referrer, console_errors, locale.
      t.jsonb :metadata, null: false, default: {}
      t.integer :github_issue_number
      t.string :github_issue_url

      t.timestamps
    end

    add_index :bug_reports, [ :workspace_id, :created_at ]
    add_index :bug_reports, :status
  end
end
