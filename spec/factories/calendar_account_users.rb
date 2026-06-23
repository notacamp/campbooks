FactoryBot.define do
  factory :calendar_account_user do
    calendar_account
    user
    can_read { true }
    can_write { false }
    can_manage { false }
    owner { false }

    # Role presets mirroring CalendarAccountUser#role (viewer/editor/manager).
    trait :viewer do
      can_read { true }
      can_write { false }
      can_manage { false }
    end

    trait :editor do
      can_read { true }
      can_write { true }
      can_manage { false }
    end

    trait :manager do
      can_read { true }
      can_write { true }
      can_manage { true }
    end

    trait :owner do
      owner { true }
      can_read { true }
      can_write { true }
      can_manage { true }
    end
  end
end
