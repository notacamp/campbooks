# frozen_string_literal: true

module People
  # One row in the People list: a person or an organization, with its display
  # bits and Scout's standing precomputed by PeopleController so the row component
  # (Campbooks::People::CounterpartRow) renders without extra queries.
  #
  # `subtitle` is the second line — a person's organization name, or an org's
  # "Organization · N people · N services". `standing` is a People::Standing::Result.
  Counterpart = Data.define(
    :kind, :record, :name, :subtitle, :avatar_email, :avatar_initial,
    :last_activity, :standing
  ) do
    def id = record.id
    def person? = kind == :person
    def organization? = kind == :organization
    def needs_you? = standing.needs_you
    def overdue_days = standing.overdue_days
  end
end
