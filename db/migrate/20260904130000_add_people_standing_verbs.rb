# frozen_string_literal: true

# Adds the verb/subject/timing columns so people_standings can carry the full
# "Need you" projection from People::Attention (feed items cut by person) and the
# standing message/item links for PR 3's row-level actions.
class AddPeopleStandingVerbs < ActiveRecord::Migration[8.1]
  def change
    add_column :people_standings, :verb,             :string
    add_column :people_standings, :subject,          :string
    add_column :people_standings, :wait_days,        :integer, default: 0, null: false
    add_column :people_standings, :feed_item_id,     :uuid
    add_column :people_standings, :email_message_id, :uuid

    add_index :people_standings, :feed_item_id
  end
end
