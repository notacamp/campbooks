FactoryBot.define do
  factory :external_service_call do
    service          { "google_mail" }
    status           { :success }
    operation        { "GET /gmail/v1/users/me/messages" }
    duration_ms      { 120 }
    request_headers  { nil }
    response_headers { nil }
    request_body     { nil }
    response_body    { nil }

    trait :error do
      status        { :error }
      http_status   { 500 }
      error_class   { "Faraday::ServerError" }
      error_message { "the server responded with status 500" }
    end

    trait :with_capture do
      request_headers  { { "Content-Type" => "application/json" } }
      response_headers { { "X-Request-Id" => "test-req-id" } }
      request_body     { '{"model":"gpt-4"}' }
      response_body    { '{"choices":[]}' }
    end
  end
end
