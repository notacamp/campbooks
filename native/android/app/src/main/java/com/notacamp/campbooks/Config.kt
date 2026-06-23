package com.notacamp.campbooks

object Config {
    // Where the app loads from. Defaults to production. For the Android emulator
    // against a local Rails server use "http://10.0.2.2:3000" (10.0.2.2 is the
    // host machine from the emulator; cleartext is allowed for it in
    // res/xml/network_security_config.xml).
    const val BASE_URL = "https://app.campbooks.not-a-camp.com"

    const val START_LOCATION = BASE_URL

    val REMOTE_PATH_CONFIGURATION_URL = "$BASE_URL/configurations/android_v1.json"

    // OAuth handoff (see OAuthRouteDecisionHandler + the Rails OauthNativeHandoff
    // concern). Must match the scheme in AndroidManifest and the Rails redirect.
    const val OAUTH_CALLBACK_SCHEME = "campbooks"

    // Provider hosts that must open in a Custom Tab (a real browser), never the
    // embedded web view — providers reject embedded webviews.
    val OAUTH_PROVIDER_HOSTS = setOf(
        "accounts.google.com",
        "login.microsoftonline.com",
        "accounts.zoho.eu",
        "accounts.zoho.com"
    )
}
