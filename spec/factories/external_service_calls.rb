FactoryBot.define do
  factory :external_service_call do
    service     { "google_mail" }
    status      { :success }
    operation   { "GET /gmail/v1/users/me/messages" }
    duration_ms { 120 }

    trait :error do
      status       { :error }
      http_status  { 500 }
      error_class  { "Faraday::ServerError" }
      error_message { "the server responded with status 500" }
    end
  end
end
