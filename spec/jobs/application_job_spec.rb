require "rails_helper"

RSpec.describe ApplicationJob, type: :job do
  # A throwaway job to exercise the around_perform metrics hook without
  # depending on any real job's side effects.
  before do
    stub_const("MetricsProbeJob", Class.new(ApplicationJob) do
      def perform(should_fail: false)
        raise "boom" if should_fail
      end
    end)
  end

  it "counts a successful run tagged by job class and outcome" do
    expect { MetricsProbeJob.perform_now }
      .to increment_yabeda_counter(Yabeda.campbooks.job_executions_total)
      .with_tags(job: "MetricsProbeJob", status: "success").by(1)
  end

  it "measures the run duration" do
    expect { MetricsProbeJob.perform_now }
      .to measure_yabeda_histogram(Yabeda.campbooks.job_duration)
      .with_tags(job: "MetricsProbeJob", status: "success")
  end

  it "counts a failed run and re-raises so retry_on/discard_on still fire" do
    expect {
      expect { MetricsProbeJob.perform_now(should_fail: true) }.to raise_error("boom")
    }.to increment_yabeda_counter(Yabeda.campbooks.job_executions_total)
      .with_tags(job: "MetricsProbeJob", status: "failure").by(1)
  end
end
