FactoryBot.define do
  factory :pipeline do
    workspace
    sequence(:name) { |n| "Pipeline #{n}" }
    applies_to { :both }

    # A pipeline with two stages (one terminal) ready to receive items.
    trait :with_stages do
      after(:create) do |pipeline|
        create(:pipeline_stage, pipeline: pipeline, name: "To do", position: 1)
        create(:pipeline_stage, pipeline: pipeline, name: "Done", position: 2, is_terminal: true)
      end
    end
  end

  factory :pipeline_stage do
    pipeline
    sequence(:name) { |n| "Stage #{n}" }
    sequence(:position) { |n| n }
    color { "#6366f1" }
    is_terminal { false }
  end

  factory :pipeline_membership do
    pipeline
    association :item, factory: :document
  end
end
