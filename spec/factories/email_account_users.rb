FactoryBot.define do
  factory :email_account_user do
    email_account
    user
    can_read { true }
    can_send { false }
    can_manage { false }
    owner { false }

    # Role presets mirroring EmailAccountUser#role (viewer/collaborator/manager).
    trait :viewer do
      can_read { true }
      can_send { false }
      can_manage { false }
    end

    trait :collaborator do
      can_read { true }
      can_send { true }
      can_manage { false }
    end

    trait :manager do
      can_read { true }
      can_send { true }
      can_manage { true }
    end

    trait :owner do
      owner { true }
      can_read { true }
      can_send { true }
      can_manage { true }
    end
  end
end
