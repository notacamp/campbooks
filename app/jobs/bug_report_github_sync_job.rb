# Mirrors a BugReport into a GitHub issue so reports land in the team's normal
# triage flow. Entirely optional: with no GITHUB_TOKEN + repo configured this is
# a no-op and the local record stands on its own. Idempotent — a report that
# already has an issue number is skipped.
#
# Configure with:
#   GITHUB_TOKEN              a PAT (or fine-grained token) with `issues: write`
#   GITHUB_BUG_REPORT_REPO    "owner/repo" to open issues in
#                             (falls back to GITHUB_REPOSITORY)
class BugReportGithubSyncJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  GITHUB_API_BASE = "https://api.github.com"

  # True only when both a token and a target repo are present, so the controller
  # can skip enqueuing pointless no-op jobs.
  def self.configured?
    ENV["GITHUB_TOKEN"].present? && repo.present?
  end

  def self.repo
    (ENV["GITHUB_BUG_REPORT_REPO"] || ENV["GITHUB_REPOSITORY"]).presence
  end

  def perform(bug_report_id)
    bug_report = BugReport.find_by(id: bug_report_id)
    return if bug_report.nil? || bug_report.synced_to_github?
    return unless self.class.configured?

    result = Workflows::HttpClient.call(
      method: :post,
      url: "#{GITHUB_API_BASE}/repos/#{self.class.repo}/issues",
      headers: {
        "Authorization" => "Bearer #{ENV.fetch('GITHUB_TOKEN')}",
        "Accept" => "application/vnd.github+json",
        "X-GitHub-Api-Version" => "2022-11-28",
        "Content-Type" => "application/json",
        "User-Agent" => "Campbooks-BugReporter"
      },
      body: issue_payload(bug_report).to_json
    )

    raise "GitHub issue creation failed (status #{result[:status]}): #{result[:error] || result[:body]}" unless result[:ok]

    issue = JSON.parse(result[:body])
    bug_report.update!(github_issue_number: issue["number"], github_issue_url: issue["html_url"])
  end

  private

  def issue_payload(bug_report)
    {
      title: bug_report.issue_title,
      body: issue_body(bug_report),
      labels: [ "bug", "user-report" ]
    }
  end

  def issue_body(bug_report)
    reporter = bug_report.user
    rows = [
      [ "Reporter", "#{reporter&.name} (#{reporter&.email_address})" ],
      [ "Workspace", "#{bug_report.workspace.name} (##{bug_report.workspace_id})" ],
      [ "Page", bug_report.page_url ],
      [ "Viewport", "#{bug_report.context('viewport')} (#{bug_report.context('breakpoint')})" ],
      [ "User agent", bug_report.user_agent ],
      [ "Locale", bug_report.context("locale") ],
      [ "Reported at", bug_report.created_at.iso8601 ],
      [ "In-app record", "BugReport ##{bug_report.id}" ]
    ]

    lines = [ bug_report.description.to_s.strip, "", "---", "", "| Context | |", "| --- | --- |" ]
    rows.each { |label, value| lines << "| #{label} | #{cell(value)} |" }

    console_errors = Array(bug_report.context("console_errors")).first(20)
    if console_errors.any?
      lines += [ "", "**Recent console errors:**", "", "```" ]
      console_errors.each { |error| lines << error.to_s }
      lines << "```"
    end

    if bug_report.screenshot.attached?
      lines += [ "", "_A screenshot is attached to the in-app record (BugReport ##{bug_report.id})._" ]
    end

    lines.join("\n")
  end

  # Keep markdown-table cells on one line and stop stray pipes from breaking the
  # table layout.
  def cell(value)
    value.to_s.gsub("|", "\\|").gsub(/\s+/, " ").strip
  end
end
