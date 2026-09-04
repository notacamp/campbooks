# frozen_string_literal: true

# Materialized per-user standings for the People place. One row per (user,
# counterpart) — computed by People::Standings.refresh! in the background
# (People::StandingsRefreshJob), triggered after every feed refresh and on
# sender-kind backfill completion. The People list reads paginated rows from
# this table instead of recomputing standings on every request.
class CreatePeopleStandings < ActiveRecord::Migration[8.1]
  def change
    create_table :people_standings, id: :uuid do |t|
      t.references :workspace, null: false, foreign_key: true, type: :uuid
      t.references :user, null: false, foreign_key: true, type: :uuid, index: false
      t.string  :counterpart_type, null: false       # "Person" | "Organization"
      t.uuid    :counterpart_id, null: false
      t.boolean :needs_you, null: false, default: false
      t.string  :standing_kind, null: false, default: "none"   # you_owe nudge prompt summary last_exchange none
      t.text    :text                                # Scout's sentence (People::Standing::Result#text)
      t.references :email_thread, null: true, type: :uuid, index: false,
                   foreign_key: { on_delete: :nullify }   # the thread behind the standing (Result#thread_id)
      t.integer :overdue_days, null: false, default: 0
      t.float   :score, null: false, default: 0.0     # People::Priority::Score#value
      t.float   :strength, null: false, default: 0.0  # People::Priority::Score#strength
      t.datetime :last_activity_at
      t.string  :name, null: false                    # display bits so the row renders with NO other loads
      t.string  :subtitle
      t.string  :avatar_email
      t.string  :avatar_initial
      t.jsonb   :data, null: false, default: {}       # { "people_count" =>, "services_count" => } for orgs
      t.datetime :refreshed_at, null: false
      t.timestamps
    end
    add_index :people_standings, %i[user_id counterpart_type counterpart_id], unique: true,
              name: "index_people_standings_on_user_counterpart"
    add_index :people_standings, %i[user_id needs_you score last_activity_at],
              order: { score: :desc, last_activity_at: :desc }, name: "index_people_standings_list"
    add_index :people_standings, %i[counterpart_type counterpart_id], name: "index_people_standings_on_counterpart"
  end
end
